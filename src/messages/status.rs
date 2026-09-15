use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveTime, Timelike as _, Utc};
use chrono_tz::Tz;
use webuntis::Homework;

use crate::{
    message::{Bold, Each, Either, Fragment, Italic, Line, Newline, Render, Text},
    messages::{DATE_FORMAT, MIDDLE_DOT, join_present, time_range},
};

#[derive(Debug, Clone)]
pub(crate) struct StatusMessage {
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
    pub(crate) fn new(
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

    pub fn into_document(self) -> impl Fragment {
        self.into_document_at(Utc::now())
    }

    fn into_document_at(self, now: DateTime<Utc>) -> impl Fragment {
        let local_now = now.with_timezone(&self.timezone);
        let today = local_now.date_naive();

        let nearest_lesson = self
            .current_or_next_lesson
            .map(|nearest| NearestLessonView::new(nearest, today));
        let homework =
            HomeworkSection(self.homeworks.into_iter().map(HomeworkView::from).collect());
        let footer = StatusFooterView {
            refreshed_at: local_now.format("%H:%M").to_string(),
            uptime: into_uptime(
                (now - self.timestamp)
                    .to_std()
                    .unwrap_or(Duration::from_secs(0)),
            ),
        };

        (nearest_lesson, homework, footer)
    }
}

struct NearestLessonView {
    heading: &'static str,
    subjects: String,
    time: String,
    date: Option<String>,
    teachers: String,
    rooms: String,
}

impl NearestLessonView {
    fn new(nearest: NearestLesson, today: NaiveDate) -> Self {
        let (heading, lesson) = match nearest {
            NearestLesson::Current(lesson) => ("Now", lesson),
            NearestLesson::Next(lesson) => ("Next", lesson),
        };

        Self {
            heading,
            subjects: lesson.subjects.join(", "),
            time: time_range(lesson.start_time, lesson.end_time),
            date: (lesson.date != today).then(|| lesson.date.format(DATE_FORMAT).to_string()),
            teachers: lesson.teachers.join(", "),
            rooms: lesson.rooms.join(", "),
        }
    }
}

impl Fragment for NearestLessonView {
    fn parts(self) -> impl Render {
        let summary = join_present([
            self.subjects.as_str(),
            self.time.as_str(),
            self.date.as_deref().unwrap_or_default(),
        ]);
        let people_and_room = join_present([self.teachers.as_str(), self.rooms.as_str()]);

        (
            Line(Bold(Text(self.heading))),
            summary.map(|summary| Line(Text(summary))),
            people_and_room.map(|people_and_room| Line(Text(people_and_room))),
            Newline,
        )
            .parts()
    }
}

struct HomeworkSection(Vec<HomeworkView>);

impl Fragment for HomeworkSection {
    fn parts(self) -> impl Render {
        let entries = if self.0.is_empty() {
            Either::Left((Line(Text("No pending homework.")), Newline))
        } else {
            Either::Right(Each(self.0))
        };

        (Line(Bold(Text("Homework"))), entries).parts()
    }
}

struct HomeworkView {
    due: String,
    subject: String,
    task: String,
}

impl From<Homework> for HomeworkView {
    fn from(homework: Homework) -> Self {
        Self {
            due: homework.due_date.format(DATE_FORMAT).to_string(),
            subject: homework.lesson.subject,
            task: homework.text,
        }
    }
}

impl Fragment for HomeworkView {
    fn parts(self) -> impl Render {
        (
            Line(Bold((Text(self.due), Text(MIDDLE_DOT), Text(self.subject)))),
            Line(Text(self.task)),
            Newline,
        )
            .parts()
    }
}

struct StatusFooterView {
    refreshed_at: String,
    uptime: String,
}

impl Fragment for StatusFooterView {
    fn parts(self) -> impl Render {
        Line(Italic(Text(format!(
            "Updated {} · up {}",
            self.refreshed_at, self.uptime
        ))))
        .parts()
    }
}

fn into_uptime(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    let days = minutes / 1_440;
    let hours = minutes % 1_440 / 60;
    let minutes = minutes % 60;
    match (days, hours) {
        (0, 0) => format!("{minutes}m"),
        (0, hours) => format!("{hours}h {minutes}m"),
        (days, hours) => format!("{days}d {hours}h"),
    }
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

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;

    use super::*;
    use crate::{
        message::render_public,
        test_support::{Golden, assert_golden, date, db_lesson, homework, untis_lesson},
    };

    // 10:15:30 in Europe/Berlin
    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2025, 9, 16, 8, 15, 30).unwrap()
    }

    fn status(
        homeworks: Vec<Homework>,
        current_or_next_lesson: Option<NearestLesson>,
    ) -> StatusMessage {
        StatusMessage {
            homeworks,
            current_or_next_lesson,
            timestamp: now() - chrono::Duration::minutes(3 * 24 * 60 + 5 * 60 + 1),
            timezone: chrono_tz::Europe::Berlin,
        }
    }

    fn current_lesson() -> db::models::Lesson {
        db_lesson(&untis_lesson(
            1,
            date(2025, 9, 16),
            10,
            11,
            "Mathematics",
            "Smith",
            "R.101",
        ))
    }

    fn next_lesson() -> db::models::Lesson {
        let mut lesson = untis_lesson(2, date(2025, 9, 17), 8, 9, "Biology-2", "Müller", "B.2");
        lesson.rooms.clear();
        db_lesson(&lesson)
    }

    fn physics_homework() -> Homework {
        homework(
            7,
            "Physics",
            "Johnson",
            "Read p. 12-14 (`notes`)",
            date(2025, 9, 10),
            date(2025, 9, 18),
        )
    }

    fn history_homework() -> Homework {
        homework(
            8,
            "History",
            "Brown",
            "Essay: 1789!",
            date(2025, 9, 12),
            date(2025, 9, 19),
        )
    }

    fn render(status: StatusMessage) -> String {
        render_public(status.into_document_at(now())).into_string()
    }

    fn assert_status_golden(name: Golden, status: StatusMessage) {
        assert_golden(name, &render(status));
    }

    #[test]
    fn golden_status_with_current_lesson() {
        assert_status_golden(
            Golden::StatusCurrentLesson,
            status(
                vec![physics_homework()],
                Some(NearestLesson::Current(current_lesson())),
            ),
        );
    }

    #[test]
    fn golden_status_with_next_lesson() {
        assert_status_golden(
            Golden::StatusNextLesson,
            status(
                vec![physics_homework()],
                Some(NearestLesson::Next(next_lesson())),
            ),
        );
    }

    #[test]
    fn golden_status_without_homework() {
        assert_status_golden(Golden::StatusNoHomework, status(vec![], None));
    }

    #[test]
    fn golden_status_with_multiple_homework() {
        assert_status_golden(
            Golden::StatusMultipleHomework,
            status(vec![physics_homework(), history_homework()], None),
        );
    }

    #[test]
    fn status_without_teacher_and_room_has_no_dangling_separator() {
        let mut lesson = current_lesson();
        lesson.teachers.clear();
        lesson.rooms.clear();

        let rendered = render(status(vec![], Some(NearestLesson::Current(lesson))));

        assert!(rendered.starts_with("*Now*\nMathematics · 10:00–11:00\n\n*Homework*\n"));
        assert!(!rendered.contains(" · \n"));
        assert!(!rendered.contains("Teacher"));
        assert!(!rendered.contains("Room"));
    }

    #[test]
    fn uptime_is_compact() {
        assert_eq!(into_uptime(Duration::from_secs(59)), "0m");
        assert_eq!(
            into_uptime(Duration::from_secs(2 * 3_600 + 5 * 60)),
            "2h 5m"
        );
        assert_eq!(
            into_uptime(Duration::from_secs(3 * 86_400 + 5 * 3_600 + 60)),
            "3d 5h"
        );
    }
}
