use chrono::NaiveDate;
use db::models::LessonCode;

use crate::{
    diff_impl::Diff,
    message::{
        Bold, CodeBlock, DebugBlock, Each, Either, Fragment, Line, Newline, Render, Strike, Text,
    },
    messages::{join_present, time_range},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LessonView {
    pub lesson_id: i64,
    pub date: NaiveDate,
    pub subjects: String,
    pub time: String,
    pub teachers: String,
    pub rooms: String,
    pub status: LessonCode,
    pub info: String,
}

impl LessonView {
    fn from_db(lesson: &db::models::Lesson) -> Self {
        Self {
            lesson_id: lesson.lesson_id,
            date: lesson.date,
            subjects: lesson.subjects.join(", "),
            time: time_range(lesson.start_time, lesson.end_time),
            teachers: lesson.teachers.join(", "),
            rooms: lesson.rooms.join(", "),
            status: lesson.lesson_code,
            info: lesson.subst_text.clone().unwrap_or_default(),
        }
    }

    fn from_webuntis(lesson: &webuntis::Lesson) -> Self {
        Self {
            lesson_id: lesson.id as i64,
            date: lesson.date.0,
            subjects: join_names(&lesson.subjects),
            time: time_range(lesson.start_time.0, lesson.end_time.0),
            teachers: join_names(&lesson.teachers),
            rooms: join_names(&lesson.rooms),
            status: lesson.code,
            info: lesson.subst_text.clone().unwrap_or_default(),
        }
    }
}

fn join_names(items: &[webuntis::IdItem]) -> String {
    items
        .iter()
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn status_name(status: LessonCode) -> &'static str {
    match status {
        LessonCode::Regular => "regular",
        LessonCode::Irregular => "irregular",
        LessonCode::Cancelled => "cancelled",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValueDelta {
    Added(String),
    Removed(String),
    Replaced { from: String, to: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldDelta {
    pub label: &'static str,
    pub value: ValueDelta,
}

fn delta(label: &'static str, from: String, to: String) -> Option<FieldDelta> {
    let value = match (from.is_empty(), to.is_empty()) {
        (true, true) => return None,
        (true, false) => ValueDelta::Added(to),
        (false, true) => ValueDelta::Removed(from),
        (false, false) if from == to => return None,
        (false, false) => ValueDelta::Replaced { from, to },
    };
    Some(FieldDelta { label, value })
}

impl Fragment for FieldDelta {
    fn parts(self) -> impl Render {
        let value = match self.value {
            ValueDelta::Added(value) => Either::Left(Text(value)),
            ValueDelta::Removed(value) => Either::Right(Either::Left(Strike(Text(value)))),
            ValueDelta::Replaced { from, to } => {
                Either::Right(Either::Right((Text(from), Text(" → "), Text(to))))
            }
        };
        Line((Text(self.label), Text(": "), value)).parts()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LessonMessageKind {
    Added,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LessonMessage {
    pub kind: LessonMessageKind,
    pub lesson: LessonView,
    pub changes: Vec<FieldDelta>,
}

/// For a changed lesson, `lesson` holds the new values and `changes` keeps the old ones.
pub(crate) fn lesson_message(diff: Diff<'_>) -> LessonMessage {
    match diff {
        Diff::Added(lesson) => LessonMessage {
            kind: LessonMessageKind::Added,
            lesson: LessonView::from_webuntis(lesson),
            changes: Vec::new(),
        },
        Diff::Changed { from, to } => {
            let from = LessonView::from_db(from);
            let to = LessonView::from_webuntis(to);
            let changes = [
                delta("Date", from.date.to_string(), to.date.to_string()),
                delta("Subject", from.subjects, to.subjects.clone()),
                delta("Time", from.time, to.time.clone()),
                delta("Teacher", from.teachers, to.teachers.clone()),
                delta("Room", from.rooms, to.rooms.clone()),
                delta(
                    "Status",
                    status_name(from.status).to_string(),
                    status_name(to.status).to_string(),
                ),
                delta("Info", from.info, to.info.clone()),
            ]
            .into_iter()
            .flatten()
            .collect();

            LessonMessage {
                kind: LessonMessageKind::Changed,
                lesson: to,
                changes,
            }
        }
    }
}

impl LessonMessage {
    fn title(&self) -> &'static str {
        match (self.kind, self.lesson.status) {
            (LessonMessageKind::Changed, _) => "🔄 Lesson changed",
            (LessonMessageKind::Added, LessonCode::Cancelled) => "❌ Cancelled lesson",
            (LessonMessageKind::Added, LessonCode::Irregular) => "⚠️ Irregular lesson",
            (LessonMessageKind::Added, LessonCode::Regular) => "➕ New lesson",
        }
    }
}

impl Fragment for LessonMessage {
    fn parts(self) -> impl Render {
        let title = self.title();
        let lesson = self.lesson;
        let added = self.kind == LessonMessageKind::Added;

        let summary = join_present([lesson.subjects.as_str(), lesson.time.as_str()]);
        let people_and_room = added
            .then(|| join_present([lesson.teachers.as_str(), lesson.rooms.as_str()]))
            .flatten();
        let added_info =
            (added && !lesson.info.is_empty()).then_some(Line((Text("Info: "), Text(lesson.info))));

        (
            Line(Bold(Text(title))),
            summary.map(|summary| Line(Text(summary))),
            people_and_room.map(|people_and_room| Line(Text(people_and_room))),
            Each(self.changes),
            added_info,
        )
            .parts()
    }
}

/// Stable troubleshooting fields shown under a lesson in the diagnostic rendering.
pub(crate) struct DiagnosticInfo {
    kind: &'static str,
    lesson_id: i64,
    date: NaiveDate,
    changed_fields: Vec<String>,
}

impl From<&LessonMessage> for DiagnosticInfo {
    fn from(message: &LessonMessage) -> Self {
        Self {
            kind: match message.kind {
                LessonMessageKind::Added => "added",
                LessonMessageKind::Changed => "changed",
            },
            lesson_id: message.lesson.lesson_id,
            date: message.lesson.date,
            changed_fields: message
                .changes
                .iter()
                .map(|change| change.label.to_lowercase().replace(' ', "_"))
                .collect(),
        }
    }
}

impl Fragment for DiagnosticInfo {
    fn parts(self) -> impl Render {
        DebugBlock((
            Newline,
            Line(Bold(Text("Debug"))),
            CodeBlock {
                code: format!(
                    "kind={}\nlesson_id={}\ndate={}\nchanged_fields={}",
                    self.kind,
                    self.lesson_id,
                    self.date,
                    self.changed_fields.join(",")
                ),
                language: None,
            },
            Newline,
        ))
        .parts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        message::{render_both, render_public},
        test_support::{Golden, assert_golden, date, db_lesson, id_item, untis_lesson},
    };

    fn render(diff: Diff<'_>) -> String {
        render_public(lesson_message(diff)).into_string()
    }

    fn changed_math() -> (db::models::Lesson, webuntis::Lesson) {
        let original = untis_lesson(
            202,
            date(2025, 9, 18),
            10,
            11,
            "Mathematics",
            "Smith",
            "R.101",
        );
        let mut to = untis_lesson(
            202,
            date(2025, 9, 18),
            10,
            12,
            "Mathematics",
            "Johnson",
            "R.101",
        );
        to.rooms.clear();
        to.subst_text = Some("Exam (room tba)".to_string());
        (db_lesson(&original), to)
    }

    #[test]
    fn golden_added_lesson() {
        let mut lesson = untis_lesson(201, date(2025, 9, 18), 8, 9, "English", "Taylor", "E.1");
        lesson.subst_text = Some("Bring dictionary!".to_string());
        assert_golden(Golden::LessonAdded, &render(Diff::Added(&lesson)));
    }

    #[test]
    fn golden_changed_lesson() {
        let (from, to) = changed_math();
        assert_golden(
            Golden::LessonChanged,
            &render(Diff::Changed {
                from: &from,
                to: &to,
            }),
        );
    }

    #[test]
    fn golden_cancelled_lesson() {
        let mut lesson = untis_lesson(
            203,
            date(2025, 9, 17),
            12,
            13,
            "Chemistry",
            "Klein",
            "Lab-1",
        );
        lesson.code = LessonCode::Cancelled;
        assert_golden(Golden::LessonCancelled, &render(Diff::Added(&lesson)));
    }

    #[test]
    fn golden_lesson_with_multiple_subjects() {
        let mut lesson = untis_lesson(204, date(2025, 9, 18), 8, 9, "Mathematics", "Taylor", "E.1");
        lesson.subjects.push(id_item(11, "Physics"));
        assert_golden(
            Golden::LessonMultipleSubjects,
            &render(Diff::Added(&lesson)),
        );
    }

    #[test]
    fn irregular_lesson_has_its_own_title() {
        let mut lesson = untis_lesson(205, date(2025, 9, 18), 8, 9, "Art", "Lee", "A.3");
        lesson.code = LessonCode::Irregular;
        assert!(render(Diff::Added(&lesson)).starts_with("*⚠️ Irregular lesson*\n"));
    }

    #[test]
    fn regular_status_is_not_printed() {
        let lesson = untis_lesson(206, date(2025, 9, 18), 8, 9, "Art", "Lee", "A.3");
        let (from, to) = changed_math();

        assert!(!render(Diff::Added(&lesson)).contains("Status: regular"));
        assert!(
            !render(Diff::Changed {
                from: &from,
                to: &to,
            })
            .contains("Status")
        );
    }

    #[test]
    fn changed_lesson_lists_date_and_status_changes() {
        let from = db_lesson(&untis_lesson(
            207,
            date(2025, 9, 18),
            8,
            9,
            "Art",
            "Lee",
            "A.3",
        ));
        let mut to = untis_lesson(207, date(2025, 9, 19), 8, 9, "Art", "Lee", "A.3");
        to.code = LessonCode::Cancelled;

        let rendered = render(Diff::Changed {
            from: &from,
            to: &to,
        });

        assert!(rendered.contains("Date: 2025\\-09\\-18 → 2025\\-09\\-19\n"));
        assert!(rendered.contains("Status: regular → cancelled\n"));
        assert!(!rendered.contains("Teacher"));
        assert!(!rendered.contains("Room"));
    }

    #[test]
    fn missing_teacher_or_room_leaves_no_dangling_separator() {
        let mut no_teacher = untis_lesson(208, date(2025, 9, 18), 8, 9, "Art", "Lee", "A.3");
        no_teacher.teachers.clear();
        let mut nobody = no_teacher.clone();
        nobody.rooms.clear();

        let rendered = render(Diff::Added(&no_teacher));
        assert!(rendered.ends_with("08:00–09:00\nA\\.3\n"));
        assert!(!rendered.contains(" · \n"));

        assert!(render(Diff::Added(&nobody)).ends_with("08:00–09:00\n"));
    }

    #[test]
    fn delta_skips_equal_and_empty_values() {
        assert_eq!(delta("Room", String::new(), String::new()), None);
        assert_eq!(delta("Room", "R.1".into(), "R.1".into()), None);
        assert_eq!(
            delta("Room", String::new(), "R.1".into()).map(|delta| delta.value),
            Some(ValueDelta::Added("R.1".into()))
        );
        assert_eq!(
            delta("Room", "R.1".into(), String::new()).map(|delta| delta.value),
            Some(ValueDelta::Removed("R.1".into()))
        );
        assert_eq!(
            delta("Room", "R.1".into(), "R.2".into()).map(|delta| delta.value),
            Some(ValueDelta::Replaced {
                from: "R.1".into(),
                to: "R.2".into(),
            })
        );
    }

    #[test]
    fn diagnostic_info_lists_changed_fields_without_model_dumps() {
        let (from, to) = changed_math();
        let message = lesson_message(Diff::Changed {
            from: &from,
            to: &to,
        });
        let rendered = render_both((message.clone(), DiagnosticInfo::from(&message)));

        assert_eq!(
            rendered.public.as_str(),
            render_public(message).as_str(),
            "debug details must stay out of the public rendering"
        );
        assert!(rendered.diagnostic.as_str().ends_with(
            "\n\n*Debug*\n```\nkind=changed\nlesson_id=202\ndate=2025-09-18\nchanged_fields=time,teacher,room,info\n```\n"
        ));
        assert!(!rendered.diagnostic.as_str().contains("Lesson \\{"));
    }
}
