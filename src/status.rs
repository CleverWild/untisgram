use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveTime, Timelike as _, Utc};
use chrono_tz::Tz;
use webuntis::Homework;

use crate::message::{
    Bold, Code, Each, Either, Fragment, Line, Link, Newline, Render, Spoiler, Text,
};

const REPOSITORY_URL: &str = "https://github.com/CleverWild/untisgram";
const HOMEWORK_DATE_FORMAT: &str = "%a %d/%m/%y";

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

    pub fn into_document(self) -> impl Fragment {
        self.into_document_at(Utc::now())
    }

    fn into_document_at(self, now: DateTime<Utc>) -> impl Fragment {
        let local_now = now.with_timezone(&self.timezone);
        let today = local_now.date_naive();

        let nearest_lesson = self.current_or_next_lesson.map(|nearest| match nearest {
            NearestLesson::Current(lesson) => NearestLessonPart {
                title: "Current Lesson:",
                lesson,
                today,
            },
            NearestLesson::Next(lesson) => NearestLessonPart {
                title: "Next Lesson:",
                lesson,
                today,
            },
        });
        let footer = StatusFooter {
            uptime: into_uptime(
                (now - self.timestamp)
                    .to_std()
                    .unwrap_or(Duration::from_secs(0)),
            ),
            last_refresh: local_now.format("%H:%M:%S %a %d/%m/%y").to_string(),
        };

        (nearest_lesson, HomeworkList(self.homeworks), footer)
    }
}

struct NearestLessonPart {
    title: &'static str,
    lesson: db::models::Lesson,
    today: NaiveDate,
}

impl Fragment for NearestLessonPart {
    fn parts(self) -> impl Render {
        let lesson = self.lesson;
        let teachers = (!lesson.teachers.is_empty())
            .then(|| (Text(", Teacher: "), Code(lesson.teachers.join(", "))));
        let rooms =
            (!lesson.rooms.is_empty()).then(|| (Text(", Room: "), Code(lesson.rooms.join(", "))));
        let time = format!(
            "{} - {}",
            lesson.start_time.format("%H:%M"),
            lesson.end_time.format("%H:%M")
        );
        let other_day = (lesson.date != self.today).then(|| {
            (
                Text("   "),
                Code(lesson.date.format("%a %d/%m/%y").to_string()),
            )
        });

        (
            Line(Bold(Text(self.title))),
            Line((
                Text("  Subject: "),
                Code(lesson.subjects.join(", ")),
                teachers,
                rooms,
            )),
            Line((Text("  Time: "), Code(time), other_day)),
            Newline,
        )
            .parts()
    }
}

struct HomeworkList(Vec<Homework>);

impl Fragment for HomeworkList {
    fn parts(self) -> impl Render {
        let entries = if self.0.is_empty() {
            Either::Left((Line(Code("Empty :)")), Newline))
        } else {
            Either::Right(Each(self.0.into_iter().map(HomeworkEntry)))
        };

        (Line(Bold(Text("Homework list:"))), entries).parts()
    }
}

struct HomeworkEntry(Homework);

impl Fragment for HomeworkEntry {
    fn parts(self) -> impl Render {
        let homework = self.0;
        (
            Line((
                Text("• Lesson: "),
                Code(homework.lesson.subject),
                Text(", Teacher: "),
                Code(homework.teacher.name),
            )),
            Line((
                Text("  Created at:  "),
                Code(homework.date.format(HOMEWORK_DATE_FORMAT).to_string()),
            )),
            Line((
                Text("  Deadline:     "),
                Code(homework.due_date.format(HOMEWORK_DATE_FORMAT).to_string()),
            )),
            Line((Text("  Task:  "), Code(homework.text))),
            Newline,
        )
            .parts()
    }
}

struct StatusFooter {
    uptime: String,
    last_refresh: String,
}

impl Fragment for StatusFooter {
    fn parts(self) -> impl Render {
        (
            Line((Text("Uptime:  "), Code(self.uptime))),
            Line((Text("Last refresh:  "), Code(self.last_refresh))),
            Spoiler(Line(Link {
                label: "(He keeps me in this basement full of care)",
                url: REPOSITORY_URL,
            })),
        )
            .parts()
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

    fn assert_status_golden(name: Golden, status: StatusMessage) {
        assert_golden(name, render_public(status.into_document_at(now())).as_str());
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
}
