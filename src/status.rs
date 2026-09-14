use std::time::Duration;

use chrono::{DateTime, NaiveTime, Timelike as _, Utc};
use chrono_tz::Tz;
use webuntis::Homework;

use crate::message::LabeledMessage;

const REPOSITORY_URL: &str = "https://github.com/CleverWild/untisgram";

#[derive(Debug, Clone)]
pub struct StatusMessage {
    homeworks: Vec<Homework>,
    current_or_next_lesson: Option<NearestLesson>,
    timestamp: DateTime<Utc>,
    timezone: Tz,
}

#[derive(Debug, Clone)]
enum NearestLesson {
    Current(db::models::Lesson),
    Next(db::models::Lesson),
}

impl StatusMessage {
    pub fn new(
        homeworks: Vec<Homework>,
        timetable: &[db::models::Lesson],
        uptime_since: DateTime<Utc>,
        timezone: Tz,
    ) -> Self {
        let sorted_hw = {
            let mut hw = homeworks
                .iter()
                .filter(|&h| !h.is_completed)
                .cloned()
                .collect::<Vec<_>>();
            hw.sort_by(|a, b| a.due_date.cmp(&b.due_date).then(a.date.cmp(&b.date)));
            hw
        };

        // Find nearest lesson
        let current_or_next_lesson =
            find_nearest_lesson(timetable, Utc::now().with_timezone(&timezone));

        Self {
            homeworks: sorted_hw,
            current_or_next_lesson,
            timestamp: uptime_since,
            timezone,
        }
    }

    pub fn into_message(self) -> LabeledMessage {
        let mut msg = LabeledMessage::new();

        // Show nearest lesson
        if let Some(lesson_info) = &self.current_or_next_lesson {
            match lesson_info {
                NearestLesson::Current(lesson) => {
                    msg.push_bold("Current Lesson:").nl();
                    format_nearest_lesson(&mut msg, lesson, self.timezone);
                }
                NearestLesson::Next(lesson) => {
                    msg.push_bold("Next Lesson:").nl();
                    format_nearest_lesson(&mut msg, lesson, self.timezone);
                }
            }
            msg.nl();
        }

        msg.push_bold("Homework list:").nl();

        if self.homeworks.is_empty() {
            msg.push_code_inline("Empty :)").nl().nl();
        } else {
            for hw in self.homeworks {
                msg.push("• Lesson: ")
                    .push_code_inline(hw.lesson.subject)
                    .push(", Teacher: ")
                    .push_code_inline(hw.teacher.name)
                    .nl();

                let hw_date_format = "%a %d/%m/%y";
                msg.push("  Created at:  ")
                    .push_code_inline(hw.date.format(hw_date_format).to_string())
                    .nl();
                msg.push("  Deadline:     ")
                    .push_code_inline(hw.due_date.format(hw_date_format).to_string())
                    .nl();

                msg.push("  Task:  ").push_code_inline(hw.text).nl().nl();
            }
        }

        // Added uptime calculation (days:hours:minutes)
        let elapsed = (Utc::now() - self.timestamp)
            .to_std()
            .unwrap_or(Duration::from_secs(0));
        msg.push("Uptime:  ")
            .push_code_inline(into_uptime(elapsed))
            .nl();

        msg.push("Last refresh:  ")
            .push_code_inline(
                Utc::now()
                    .with_timezone(&self.timezone)
                    .format("%H:%M:%S %a %d/%m/%y")
                    .to_string(),
            )
            .nl();

        msg.enter_spoiler(|msg| {
            msg.push_link(
                "(He keeps me in this basement full of care)",
                REPOSITORY_URL,
            )
            .nl()
        });
        msg
    }
}

fn plural(n: u64, one: &str, many: &str) -> String {
    if n == 1 {
        one.to_string()
    } else {
        many.to_string()
    }
}

fn into_uptime(d: Duration) -> String {
    let total_minutes = d.as_secs() / 60;
    let days = total_minutes / (24 * 60);
    let hours = (total_minutes % (24 * 60)) / 60;
    let minutes = total_minutes % 60;

    format!(
        "{d} {}, {h} {}, {m} {}",
        plural(days, "day", "days"),
        plural(hours, "hour", "hours"),
        plural(minutes, "minute", "minutes"),
        d = days,
        h = hours,
        m = minutes
    )
}

/// Find the current lesson (if ongoing) or the next upcoming lesson
fn find_nearest_lesson(
    timetable: &[db::models::Lesson],
    target_time: DateTime<Tz>,
) -> Option<NearestLesson> {
    // Create an iterator over non-cancelled lessons
    let lessons_iter = timetable
        .iter()
        .filter(|lesson| lesson.lesson_code != db::models::LessonCode::Cancelled);

    let date = target_time.date_naive();
    let time = NaiveTime::from_hms_opt(target_time.hour(), target_time.minute(), 0)?;

    // First, check if there's a current lesson (today and ongoing)
    if let Some(current) = lessons_iter
        .clone()
        .find(|lesson| lesson.date == date && lesson.start_time <= time && time < lesson.end_time)
    {
        return Some(NearestLesson::Current(current.clone()));
    }

    // If no current lesson, pick the next upcoming one (smallest date/time > now)
    lessons_iter
        .clone()
        .filter(|lesson| lesson.date > date || (lesson.date == date && lesson.start_time > time))
        .cloned()
        .min_by_key(|lesson| (lesson.date, lesson.start_time))
        .map(NearestLesson::Next)
}

/// Format a lesson for display in status message
fn format_nearest_lesson(msg: &mut LabeledMessage, lesson: &db::models::Lesson, timezone: Tz) {
    // Subject
    let subjects: Vec<_> = lesson.subjects.iter().map(|s| s.as_str()).collect();
    msg.push("  Subject: ")
        .push_code_inline(subjects.join(", "));

    // Teacher
    let teachers: Vec<_> = lesson.teachers.iter().map(|t| t.as_str()).collect();
    if !teachers.is_empty() {
        msg.push(", Teacher: ")
            .push_code_inline(teachers.join(", "));
    }

    // Room
    let rooms: Vec<_> = lesson.rooms.iter().map(|r| r.as_str()).collect();
    if !rooms.is_empty() {
        msg.push(", Room: ").push_code_inline(rooms.join(", "));
    }
    msg.nl();

    // Time
    msg.push("  Time: ").push_code_inline(format!(
        "{} - {}",
        lesson.start_time.format("%H:%M"),
        lesson.end_time.format("%H:%M")
    ));
    // Date (if not today)
    let today = Utc::now().with_timezone(&timezone).date_naive();
    if lesson.date != today {
        msg.push("   ")
            .push_code_inline(lesson.date.format("%a %d/%m/%y").to_string());
    }
    msg.nl();
}
