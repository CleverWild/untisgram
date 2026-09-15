//! Domain messages: lesson notifications and the status message.

mod lesson;
mod notification;
mod status;

use chrono::NaiveTime;

#[cfg(test)]
pub(crate) use lesson::lesson_message;
pub(crate) use notification::render_notification_pages;
pub(crate) use status::StatusMessage;

const DATE_FORMAT: &str = "%a, %-d %b";
const MIDDLE_DOT: &str = " · ";

fn time_range(start: NaiveTime, end: NaiveTime) -> String {
    format!("{}–{}", start.format("%H:%M"), end.format("%H:%M"))
}

/// Joins the non-empty values with a middle dot, so a missing value leaves no dangling separator.
fn join_present<'a>(values: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let present: Vec<&str> = values
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect();
    (!present.is_empty()).then(|| present.join(MIDDLE_DOT))
}
