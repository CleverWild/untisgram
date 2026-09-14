use std::{convert::Infallible, fmt, marker::PhantomData, time::Duration};

use chrono::NaiveDate;
use teloxide::{Bot, prelude::ChatId};
use tracing::Instrument;

use crate::{
    DEBUG_TELEGRAM_CHAT, IS_PROD,
    diff_impl::Diff,
    message::{
        DebugBlock, Diagnostic, Each, Either, Fragment, Newline, Public, Render, Separator, Text,
        render_both, render_public,
    },
    message_formatter::{DebugInfo, format_message},
    utils::{next_friday, send_or_edit_message, sort_diffs},
};
use db::{DateTime, Utc, models::BotTask};

pub struct Chat<A> {
    pub id: ChatId,
    pub thread_id: Option<i32>,
    audience: PhantomData<A>,
}

impl Chat<Public> {
    pub const fn public(id: ChatId, thread_id: Option<i32>) -> Self {
        Self {
            id,
            thread_id,
            audience: PhantomData,
        }
    }
}

impl Chat<Diagnostic> {
    pub const fn diagnostic(id: ChatId, thread_id: Option<i32>) -> Self {
        Self {
            id,
            thread_id,
            audience: PhantomData,
        }
    }
}

impl<A> Clone for Chat<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A> Copy for Chat<A> {}

impl<A> fmt::Debug for Chat<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chat")
            .field("id", &self.id)
            .field("thread_id", &self.thread_id)
            .finish()
    }
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

            let rendered = render_both(Notification(diffs));

            if IS_PROD {
                // Send message to production target
                tracing::debug!(
                    "Sending to chat `{}` with topic `{:?}` message:\n{}",
                    notification_chat_id,
                    notification_thread_id,
                    rendered.diagnostic
                );
                if let Err(e) = send_or_edit_message(
                    &ctx.bot,
                    Chat::public(ChatId(*notification_chat_id), *notification_thread_id),
                    rendered.public,
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
                rendered.diagnostic,
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

    let status_message = render_public(
        crate::status::StatusMessage::new(homeworks, &timetable, ctx.engaged_at, tz)
            .into_document(),
    );

    if let Err(e) = send_or_edit_message(
        &ctx.bot,
        Chat::public(ChatId(ctx.task.status_chat_id), ctx.task.status_thread_id),
        status_message,
        &mut ctx.task.status_message_id,
    )
    .await
    {
        tracing::error!("Failed to send/edit status message: {e}");
    }

    Ok(())
}

/// All lesson changes of one timetable refresh, grouped by date.
struct Notification<'a>(Vec<Diff<'a>>);

impl<'a> Fragment for Notification<'a> {
    fn parts(self) -> impl Render {
        let mut previous_date = None;
        let changes = self.0.into_iter().map(move |diff| {
            let date = diff.date();
            let heading = if previous_date == Some(date) {
                Either::Right(Separator)
            } else {
                previous_date = Some(date);
                Either::Left(DateHeading(date))
            };
            LessonChange { heading, diff }
        });

        Each(changes).parts()
    }
}

struct DateHeading(NaiveDate);

impl Fragment for DateHeading {
    fn parts(self) -> impl Render {
        let date = self.0;
        (
            Newline,
            Text(format!("Date: {date} {}", date.format("%A"))),
            Newline,
        )
            .parts()
    }
}

struct LessonChange<'a> {
    heading: Either<DateHeading, Separator>,
    diff: Diff<'a>,
}

impl<'a> Fragment for LessonChange<'a> {
    fn parts(self) -> impl Render {
        (
            self.heading,
            format_message(self.diff.clone()),
            Newline,
            DebugBlock((format_message(self.diff.clone()), DebugInfo(self.diff))),
        )
            .parts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Golden, assert_golden, date, db_lesson, untis_lesson};

    struct Fixture {
        added_first_day: webuntis::Lesson,
        changed_from: db::models::Lesson,
        changed_to: webuntis::Lesson,
        added_second_day: webuntis::Lesson,
    }

    fn fixture() -> Fixture {
        let original = untis_lesson(
            102,
            date(2025, 9, 16),
            10,
            11,
            "Mathematics",
            "Smith",
            "R.101",
        );
        let mut changed_to = untis_lesson(
            102,
            date(2025, 9, 16),
            10,
            11,
            "Mathematics",
            "Johnson",
            "R.202",
        );
        changed_to.subst_text = Some("Room swap (see board)".to_string());
        let mut added_second_day = untis_lesson(
            103,
            date(2025, 9, 17),
            12,
            13,
            "Chemistry",
            "Klein",
            "Lab-1",
        );
        added_second_day.code = db::models::LessonCode::Cancelled;

        Fixture {
            added_first_day: untis_lesson(101, date(2025, 9, 16), 8, 9, "Biology", "Weber", "B.2"),
            changed_from: db_lesson(&original),
            changed_to,
            added_second_day,
        }
    }

    fn diffs(fixture: &Fixture) -> Vec<Diff<'_>> {
        vec![
            Diff::Added(&fixture.added_first_day),
            Diff::Changed {
                from: &fixture.changed_from,
                to: &fixture.changed_to,
            },
            Diff::Added(&fixture.added_second_day),
        ]
    }

    #[test]
    fn golden_notification() {
        let fixture = fixture();
        let rendered = render_both(Notification(diffs(&fixture)));
        assert_golden(Golden::NotificationPublic, rendered.public.as_str());
        assert_golden(Golden::NotificationDiagnostic, rendered.diagnostic.as_str());
    }
}
