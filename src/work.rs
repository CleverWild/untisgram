use std::{convert::Infallible, time::Duration};

use chrono::NaiveDate;
use teloxide::{Bot, prelude::ChatId};
use tracing::Instrument;

use crate::{
    DEBUG_TELEGRAM_CHAT, IS_PROD,
    diff_impl::Diff,
    message,
    message_formatter::{apply_debug_info, format_message},
    utils::{next_friday, send_or_edit_message, sort_diffs},
};
use db::{DateTime, Utc, models::BotTask};

#[derive(Debug, Clone, Copy)]
pub struct Chat {
    pub id: ChatId,
    pub thread_id: Option<i32>,
}

pub struct WorkerContext {
    pub bot: Bot,
    pub untis_client: webuntis::Client,
    pub engaged_at: DateTime<Utc>,
    pub task: BotTask,
}

#[tracing::instrument(skip_all, fields(%task = ctx.task.task_name))]
pub async fn working_loop(mut ctx: WorkerContext) -> Result<Infallible, eyre::Report> {
    let span = tracing::info_span!("preparation");

    let tz: chrono_tz::Tz = match ctx.task.timezone.parse() {
        Ok(tz) => tz,
        Err(e) => {
            let default = chrono_tz::Europe::Berlin;
            tracing::warn!(
                parent: &span,
                "Failed to parse timezone '{tz}', defaulting to {default}: {e}",
                tz = ctx.task.timezone,
            );
            ctx.task.timezone = default.to_string();
            default
        }
    };

    tracing::debug!(parent: &span, "Starting fetch");

    let mut prev: Option<Vec<db::models::Lesson>> = match db::models::Lesson::get_all() {
        Ok(db_lessons) if !db_lessons.is_empty() => Some(db_lessons),
        Ok(_) => {
            tracing::info!(parent: &span, "No timetable found in DB");
            None
        }
        Err(e) => {
            tracing::warn!(parent: &span, "Failed to load timetable from DB: {e}");
            None
        }
    };

    const DURATION: Duration = Duration::from_secs(60);

    let mut interval = tokio::time::interval_at(crate::utils::align_next_minute(), DURATION);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    for i in 0u32.. {
        let span = tracing::info_span!("iteration", i);

        let start = tokio::time::Instant::now();

        // Fetch new timetable from Untis and save to DB
        prev = Some(
            process_timetable(&mut ctx, &prev)
                .instrument(span.clone())
                .await?,
        );

        // Update status message (queries DB internally)
        update_status(&mut ctx, tz).instrument(span.clone()).await?;

        // Update bot state in database using specific state ID to avoid race conditions
        if let Err(e) = ctx.task.update(db::models::BotTaskChangeset {
            status_message_id: Some(ctx.task.status_message_id),
            target_class_name: None,
            untis_school: None,
            task_name: None,
            untis_login: None,
            untis_password: None,
            updated_at: Some(chrono::Utc::now()),
            notification_chat_id: None,
            notification_thread_id: None,
            status_chat_id: None,
            status_thread_id: None,
            timezone: None,
        }) {
            tracing::error!(parent: &span, "Failed to update bot state in DB: {e}");
        }

        // Refresh bot state from database to detect external changes
        ctx.task.reload()?;

        // Warn if time to work is to long
        let elapsed = start.elapsed();
        if elapsed > DURATION {
            tracing::warn!(parent: &span, "Iteration took longer ({elapsed:?}) than the interval");
        } else {
            tracing::trace!(parent: &span, "Iteration took {elapsed:?}");
        }

        interval.tick().await;
    }

    #[allow(dead_code)]
    const YEARS_TO_OVERFLOW: u64 = u32::MAX as u64 / (365 * 24 * 60 * 60) * DURATION.as_secs();
    unreachable!("Integer overflow is unlikely here, but possible, see <YEARS_TO_OVERFLOW> const");
}

#[tracing::instrument(skip_all)]
async fn process_timetable(
    ctx: &mut WorkerContext,
    prev: &Option<Vec<db::models::Lesson>>,
) -> Result<Vec<db::models::Lesson>, eyre::Report> {
    let BotTask {
        target_class_name,
        notification_chat_id,
        notification_thread_id,
        ..
    } = &ctx.task;

    tracing::debug!("Fetching timetable");

    let date =
        webuntis::Date(next_friday(chrono::Local::now().date_naive()) + chrono::Duration::days(7));
    let timetable = match target_class_name {
        Some(class_name) => {
            let classes = ctx.untis_client.classes().await?;
            let found_class = classes.into_iter().find(|class| class.name == *class_name);
            let id = found_class.map(|class| class.id).ok_or_else(|| {
                eyre::eyre!("Failed to find class with name {:?}", target_class_name)
            })?;

            ctx.untis_client
                .timetable_until(&id, &webuntis::ElementType::Class, &date)
                .await?
        }
        None => ctx.untis_client.own_timetable_until(&date).await?,
    };

    if let Some(prev) = prev {
        let mut diffs = Diff::find(prev, &timetable);

        if !diffs.is_empty() {
            sort_diffs(&mut diffs);

            tracing::debug!("Diff was found: {:#?}", diffs);

            let mut message = message::LabeledMessage::new();
            let mut prev_date: Option<NaiveDate> = None;

            for (i, diff) in diffs.into_iter().enumerate() {
                let date = diff.date();

                if let Some(separator) = if prev_date != Some(date) {
                    prev_date = Some(date);
                    Some(format!("\nDate: {date} {}\n", date.format("%A")))
                } else if i != 0 {
                    Some("----------------\n".to_string())
                } else {
                    None
                } {
                    message.push(separator);
                }

                let formatted = format_message(diff.clone()).into_labeled_message();

                // Add normal view
                message.extend(formatted.filter_normal());

                // Add debug view with additional debug info
                message.debug_ln(|msg| {
                    msg.extend(formatted);
                    apply_debug_info(msg, &ctx.task, &diff)
                });
            }

            if IS_PROD {
                // Send message to production target
                tracing::debug!(
                    "Sending to chat `{}` with topic `{:?}` message:\n{}",
                    notification_chat_id,
                    notification_thread_id,
                    message
                );
                if let Err(e) = send_or_edit_message(
                    &ctx.bot,
                    Chat {
                        id: ChatId(*notification_chat_id),
                        thread_id: *notification_thread_id,
                    },
                    message.filter_normal().to_string(),
                    &mut None,
                )
                .await
                {
                    tracing::error!("Failed to send/edit notification message: {e}");
                }
            }
            // Send message to debug target
            if let Err(e) = send_or_edit_message(
                &ctx.bot,
                DEBUG_TELEGRAM_CHAT,
                message.to_string(),
                &mut None,
            )
            .await
            {
                tracing::error!("Failed to send/edit debug notification message: {e}");
            }
        }
    }

    // Save timetable to database: delete old lessons and insert new ones
    tracing::debug!("Saving timetable to database");

    // Delete all existing lessons for this bot task
    if let Err(e) = ctx.task.delete_lessons() {
        tracing::warn!("Failed to delete old lessons from DB: {e}");
    }

    // Convert Untis lessons to NewLesson insertable format
    let new_lessons: Vec<db::models::NewLesson> = timetable
        .iter()
        .cloned()
        .map(|l| ctx.task.bind_lesson_to_self(l.into()))
        .collect();

    // Insert new lessons in batch
    match ctx.task.insert_lessons_batch(new_lessons) {
        Ok(new_db_lessons) => {
            tracing::debug!("Successfully saved {} lessons to database", timetable.len());
            Ok(new_db_lessons)
        }
        Err(e) => Err(eyre::eyre!("Failed to insert lessons batch to DB: {e}")),
    }
}

#[tracing::instrument(skip_all)]
async fn update_status(ctx: &mut WorkerContext, tz: chrono_tz::Tz) -> Result<(), eyre::Report> {
    let today = chrono::Local::now().date_naive();

    // Fetch current timetable from database
    let timetable = db::models::Lesson::get_owned_by_task_id(ctx.task.id).unwrap_or_else(|e| {
        tracing::warn!("Failed to load timetable from DB for status update: {e}");
        Vec::new()
    });

    if timetable.len() <= 5 {
        tracing::warn!("Number of lessons in timetable is low: {}", timetable.len());
    }

    // Filter out expired homeworks based on timetable
    // Only filter homeworks that are due TODAY and the lesson has already passed
    let homeworks: Vec<_> = ctx
        .untis_client
        .homeworks_data()
        .await?
        .into_homeworks()
        .into_iter()
        .filter(|hw| {
            // Keep all future/past homeworks as-is
            if hw.due_date.0 != today {
                return true;
            }

            // For today's homeworks, check if the subject lesson already occurred
            let subject_occurred_today = timetable.iter().filter(|l| l.date == today).any(|ls| {
                let is_same_subject = ls.subjects.contains(&hw.lesson.subject);
                let is_same_teacher = ls.teachers.contains(&hw.teacher.name);

                is_same_subject && is_same_teacher
            });

            // Keep homework only if the subject lesson hasn't occurred today
            !subject_occurred_today
        })
        .collect();

    let status_message =
        crate::status::StatusMessage::new(homeworks, &timetable, ctx.engaged_at, tz).into_message();

    // if let Some(message_id) = ctx.task.status_message_id {
    //     if let Err(e) = edit_message(
    //         &ctx.bot,
    //         Chat {
    //             id: ChatId(ctx.task.status_chat_id),
    //             thread_id: ctx.task.status_thread_id,
    //         },
    //         MessageId(message_id),
    //         status_message.to_string(),
    //     )
    //     .await
    //     {
    //         tracing::warn!("Failed to edit status message: {e}");
    //         ctx.task.status_message_id = None;
    //     }
    // } else {
    //     tracing::warn!("Re-sending status message");
    //     let msg = send_message(
    //         &ctx.bot,
    //         Chat {
    //             id: ChatId(ctx.task.status_chat_id),
    //             thread_id: ctx.task.status_thread_id,
    //         },
    //         status_message.to_string(),
    //     )
    //     .await?;
    //     ctx.task.status_message_id.replace(msg.id.0);
    // }
    if let Err(e) = send_or_edit_message(
        &ctx.bot,
        Chat {
            id: ChatId(ctx.task.status_chat_id),
            thread_id: ctx.task.status_thread_id,
        },
        status_message.to_string(),
        &mut ctx.task.status_message_id,
    )
    .await
    {
        tracing::error!("Failed to send/edit status message: {e}");
    }

    Ok(())
}
