//! Core formatting functionality - the basic message dispatcher and public API
pub mod fields;
pub mod formatters;

use crate::{
    diff_impl::Diff,
    message::{Each, Either, Fragment, Newline, Render, Text},
    message_formatter::fields::{Field, FieldDiff},
};

/// Primary entry point for formatting lesson difference messages
pub fn format_message(diff: Diff) -> LessonDiffBlock {
    let (from, to) = match diff {
        Diff::Changed { from, to } => (Some(&from.to_owned().into()), &to.to_owned().into()),
        Diff::Added(lesson) => (None, &lesson.to_owned().into()),
    };

    formatters::format_lesson_fields(from, to)
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageHeader {
    Added,
    Changed,
}

#[derive(Debug, Clone)]
pub enum MessageField {
    Normal(Field),
    Changed(FieldDiff),
}

impl Fragment for MessageField {
    fn parts(self) -> impl Render {
        match self {
            MessageField::Normal(field) => Either::Left(field),
            MessageField::Changed(diff) => Either::Right(diff),
        }
        .parts()
    }
}

#[derive(Debug, Clone)]
pub struct LessonDiffBlock {
    header: MessageHeader,
    fields: Vec<MessageField>,
}

impl LessonDiffBlock {
    pub fn new(with_type: MessageHeader, fields: Vec<MessageField>) -> Self {
        Self {
            header: with_type,
            fields,
        }
    }
}

impl Fragment for LessonDiffBlock {
    fn parts(self) -> impl Render {
        let header = match self.header {
            MessageHeader::Changed => "Changes in lesson:",
            MessageHeader::Added => "New lesson:",
        };
        let fields = self
            .fields
            .into_iter()
            .enumerate()
            .map(|(index, field)| ((index != 0).then_some(Newline), field));

        (Text(header), Newline, Each(fields)).parts()
    }
}

/// Raw lesson data appended to the diagnostic notification.
pub struct DebugInfo<'a>(pub Diff<'a>);

impl Fragment for DebugInfo<'_> {
    fn parts(self) -> impl Render {
        match self.0 {
            Diff::Changed { from, to } => Either::Left((
                Text("Debug info: Changed lesson from "),
                Text(format!("{from:?}")),
                Text(" to "),
                Text(format!("{to:?}")),
            )),
            Diff::Added(lesson) => Either::Right((
                Text("Debug info: Added new lesson: "),
                Text(format!("{lesson:?}")),
            )),
        }
        .parts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        message::render_public,
        test_support::{Golden, assert_golden, date, db_lesson, untis_lesson},
    };

    #[test]
    fn lesson_block_contains_header_and_values() {
        let fields = vec![MessageField::Normal(Field {
            name: "Subject",
            value: "Math".to_string(),
        })];
        let block = LessonDiffBlock::new(MessageHeader::Added, fields);
        assert_eq!(
            render_public(block).as_str(),
            "New lesson:\nSubject: `Math`"
        );
    }

    #[test]
    fn golden_added_lesson() {
        let mut lesson = untis_lesson(201, date(2025, 9, 18), 8, 9, "English", "Taylor", "E.1");
        lesson.subst_text = Some("Bring dictionary!".to_string());
        let block = format_message(Diff::Added(&lesson));
        assert_golden(Golden::LessonAdded, render_public(block).as_str());
    }

    #[test]
    fn golden_changed_lesson() {
        let original = untis_lesson(
            202,
            date(2025, 9, 18),
            10,
            11,
            "Mathematics",
            "Smith",
            "R.101",
        );
        let from = db_lesson(&original);
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
        let block = format_message(Diff::Changed {
            from: &from,
            to: &to,
        });
        assert_golden(Golden::LessonChanged, render_public(block).as_str());
    }

    #[test]
    fn debug_info_describes_added_lesson() {
        let lesson = untis_lesson(203, date(2025, 9, 19), 9, 10, "Art", "Lee", "A.3");
        let rendered = render_public(DebugInfo(Diff::Added(&lesson)));
        assert!(
            rendered
                .as_str()
                .starts_with("Debug info: Added new lesson: Lesson \\{")
        );
    }
}
