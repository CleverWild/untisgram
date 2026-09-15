use std::time::Duration;

use chrono::{Datelike, NaiveDate};
use teloxide::{
    Bot,
    payloads::{EditMessageTextSetters as _, SendMessageSetters as _},
    prelude::{Message, Request as _, Requester as _},
    sugar::request::RequestLinkPreviewExt,
    types::MessageId,
};
use tokio::time::Instant;

use crate::{
    diff_impl::Diff,
    message::{MAX_TELEGRAM_TEXT_CHARS, Rendered},
    work::Chat,
};

#[derive(Debug, thiserror::Error)]
#[error("rendered Telegram message has {actual} characters; limit is {limit}")]
struct MessageTooLong {
    actual: usize,
    limit: usize,
}

fn validate_message_length<A>(text: &Rendered<A>) -> Result<(), MessageTooLong> {
    let actual = text.char_count();
    if actual > MAX_TELEGRAM_TEXT_CHARS {
        return Err(MessageTooLong {
            actual,
            limit: MAX_TELEGRAM_TEXT_CHARS,
        });
    }
    Ok(())
}

pub fn next_friday(from: NaiveDate) -> NaiveDate {
    let days_to_next_friday = (11 - from.weekday().num_days_from_monday()) % 7;
    from + chrono::Duration::days(days_to_next_friday as i64)
}

pub fn sort_diffs(diffs: &mut Vec<Diff>) {
    diffs.sort_by(|l, r| {
        l.date()
            .cmp(&r.date())
            .then_with(|| l.start_time().cmp(&r.start_time()))
            .then_with(|| l.end_time().cmp(&r.end_time()))
            .then_with(|| l.code().cmp(r.code()))
    })
}

#[tracing::instrument(skip(bot))]
pub async fn send_or_edit_message<A>(
    bot: &Bot,
    chat: Chat<A>,
    text: Rendered<A>,
    message_id: &mut Option<i32>,
) -> Result<Message, eyre::Report> {
    validate_message_length(&text)?;
    let text = text.into_string();
    let message = if let Some(msg_id) = message_id {
        tracing::debug!("Editing existing message");
        bot.edit_message_text(chat.id, MessageId(*msg_id), text.clone())
            .disable_link_preview(true)
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .send()
            .await
            .map_err(|e| eyre::eyre!(e))?
    } else {
        tracing::debug!("Sending new message");
        let mut req = bot
            .send_message(chat.id, text)
            .disable_link_preview(true)
            .parse_mode(teloxide::types::ParseMode::MarkdownV2);

        if let Some(thread_id) = chat.thread_id {
            req = req.message_thread_id(teloxide::types::ThreadId(teloxide::types::MessageId(
                thread_id,
            )));
        }

        let message = req.send().await.map_err(|e| eyre::eyre!(e))?;
        *message_id = Some(message.id.0);
        message
    };

    tracing::trace!(
        "Sent/edited message: {}",
        message.text().unwrap_or("<no text>")
    );
    Ok(message)
}

pub fn align_next_minute() -> Instant {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let secs_left = Duration::from_secs(60 - now.as_secs() % 60);
    let nanos_left = secs_left - Duration::from_nanos(now.subsec_nanos() as u64);

    Instant::now() + nanos_left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Text, render_public};

    #[test]
    fn oversized_message_is_rejected_before_sending() {
        let text = render_public(Text("x".repeat(MAX_TELEGRAM_TEXT_CHARS + 1)));

        let error = validate_message_length(&text).unwrap_err().to_string();

        assert!(error.contains("4097"), "{error}");
        assert!(error.contains("4096"), "{error}");
    }

    #[test]
    fn message_of_exactly_the_limit_is_accepted() {
        let text = render_public(Text("x".repeat(MAX_TELEGRAM_TEXT_CHARS)));
        assert!(validate_message_length(&text).is_ok());
    }

    #[test]
    fn next_friday_test() {
        let date = NaiveDate::from_ymd_opt(2025, 5, 27).unwrap(); // Tuesday
        let next_friday_date = next_friday(date);
        assert_eq!(
            next_friday_date,
            NaiveDate::from_ymd_opt(2025, 5, 30).unwrap() // Friday
        );
    }
}
