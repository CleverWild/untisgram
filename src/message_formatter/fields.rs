//! Field system for extracting and formatting lesson data

use crate::message::{Code, Either, Fragment, Render, Strike, Text};

pub const FIELD_SEPARATOR: &str = ": ";
pub const CHANGES_SEPARATOR: &str = " → ";

/// Type alias for field extraction functions
pub type FieldExtractor = fn(&db::models::UnownedLesson) -> String;

/// Represents a lesson field that can be extracted and formatted
#[derive(Clone)]
pub struct LessonFieldExtractor {
    /// Human-readable name of the field
    pub name: &'static str,
    /// Function to extract the field value from a lesson
    pub extractor: FieldExtractor,
    /// Configuration for this field
    pub config: FieldVisibility,
}

/// Configuration options for lesson fields
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldVisibility {
    /// Always show this field, even if empty
    Always,
    /// Show when not empty
    NonEmpty,
    /// Show only when changed
    Changed,
}

impl LessonFieldExtractor {
    /// Creates a new required field
    pub const fn required(name: &'static str, extractor: FieldExtractor) -> Self {
        Self {
            name,
            extractor,
            config: FieldVisibility::Always,
        }
    }

    /// Creates a new optional field that always shows
    pub const fn non_empty(name: &'static str, extractor: FieldExtractor) -> Self {
        Self {
            name,
            extractor,
            config: FieldVisibility::NonEmpty,
        }
    }

    /// Creates a new optional field that only shows when changed
    pub const fn only_changed(name: &'static str, extractor: FieldExtractor) -> Self {
        Self {
            name,
            extractor,
            config: FieldVisibility::Changed,
        }
    }

    /// Extracts the field value from a lesson
    pub fn extract(&self, lesson: &db::models::UnownedLesson) -> String {
        (self.extractor)(lesson)
    }
}

pub const FIELD_EXTRACTOR_CONFIG: &[LessonFieldExtractor] = &[
    LessonFieldExtractor::required("Subject", |l| {
        l.subjects
            .first()
            .map_or(String::new(), |subject| subject.clone())
    }),
    LessonFieldExtractor::required("Time", |l| {
        format!(
            "{} - {}",
            l.start_time.format("%H:%M"),
            l.end_time.format("%H:%M")
        )
    }),
    LessonFieldExtractor::required("Teacher", |l| l.teachers.to_vec().join(", ")),
    LessonFieldExtractor::required("Room", |l| l.rooms.to_vec().join(", ")),
    LessonFieldExtractor::required("Status", |l| match l.lesson_code {
        db::models::LessonCode::Regular => "regular".to_string(),
        db::models::LessonCode::Irregular => "irregular".to_string(),
        db::models::LessonCode::Cancelled => "cancelled".to_string(),
    }),
    LessonFieldExtractor::non_empty("Additional Info", |l| {
        l.subst_text.clone().unwrap_or_default()
    }),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Name of the field
    pub name: &'static str,
    pub value: String,
}

impl Fragment for Field {
    fn parts(self) -> impl Render {
        (Text(self.name), Text(FIELD_SEPARATOR), Code(self.value)).parts()
    }
}

/// Represents a difference between two field values
///
/// Invariant: `from` and `to` are not equal
#[derive(Debug, Clone)]
pub struct FieldDiff {
    pub name: &'static str,
    pub from: String,
    pub to: String,
    pub visibility: FieldVisibility,
}

impl FieldDiff {
    /// Creates a new field difference
    pub fn new(
        name: &'static str,
        from_value: String,
        to_value: String,
        visibility: FieldVisibility,
    ) -> Option<Self> {
        if from_value != to_value {
            Some(Self {
                name,
                from: from_value,
                to: to_value,
                visibility,
            })
        } else {
            None
        }
    }

    /// Checks if this field has actual changes
    pub fn has_changes(&self) -> bool {
        self.from != self.to
    }

    fn into_change(self) -> Option<FieldChange> {
        if !self.has_changes() {
            let visible = self.visibility != FieldVisibility::Changed && !self.from.is_empty();
            return visible.then_some(FieldChange::Unchanged(UnchangedField {
                name: self.name,
                value: self.from,
            }));
        }

        match (self.from.is_empty(), self.to.is_empty()) {
            (true, false) => Some(FieldChange::Added(AddedField {
                name: self.name,
                value: self.to,
            })),
            (false, false) => Some(FieldChange::Replaced(ReplacedField {
                name: self.name,
                from: self.from,
                to: self.to,
            })),
            (false, true) => Some(FieldChange::Removed(RemovedField {
                name: self.name,
                value: self.from,
            })),
            (true, true) => {
                tracing::warn!(
                    "Unexpected field diff state: from_value='{}', to_value='{}'",
                    self.from,
                    self.to
                );
                None
            }
        }
    }

    // pub fn field_from(&self) -> Field {
    //     Field {
    //         name: self.name,
    //         value: self.from.clone(),
    //     }
    // }

    // pub fn field_to(&self) -> Field {
    //     Field {
    //         name: self.name,
    //         value: self.to.clone(),
    //     }
    // }
}

impl Fragment for FieldDiff {
    fn parts(self) -> impl Render {
        self.into_change().parts()
    }
}

enum FieldChange {
    Unchanged(UnchangedField),
    Added(AddedField),
    Replaced(ReplacedField),
    Removed(RemovedField),
}

impl Fragment for FieldChange {
    fn parts(self) -> impl Render {
        match self {
            FieldChange::Unchanged(part) => Either::Left(Either::Left(part)),
            FieldChange::Added(part) => Either::Left(Either::Right(part)),
            FieldChange::Replaced(part) => Either::Right(Either::Left(part)),
            FieldChange::Removed(part) => Either::Right(Either::Right(part)),
        }
        .parts()
    }
}

struct UnchangedField {
    name: &'static str,
    value: String,
}

impl Fragment for UnchangedField {
    fn parts(self) -> impl Render {
        (Text(self.name), Text(FIELD_SEPARATOR), Text(self.value)).parts()
    }
}

struct AddedField {
    name: &'static str,
    value: String,
}

impl Fragment for AddedField {
    fn parts(self) -> impl Render {
        (
            Text("Added "),
            Text(self.name),
            Text(FIELD_SEPARATOR),
            Code(self.value),
        )
            .parts()
    }
}

struct ReplacedField {
    name: &'static str,
    from: String,
    to: String,
}

impl Fragment for ReplacedField {
    fn parts(self) -> impl Render {
        (
            Text(self.name),
            Text(FIELD_SEPARATOR),
            Code(self.from),
            Text(CHANGES_SEPARATOR),
            Code(self.to),
        )
            .parts()
    }
}

struct RemovedField {
    name: &'static str,
    value: String,
}

impl Fragment for RemovedField {
    fn parts(self) -> impl Render {
        (
            Text(self.name),
            Text(FIELD_SEPARATOR),
            Strike(Code(self.value)),
        )
            .parts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::render_public;

    fn render(diff: FieldDiff) -> String {
        render_public(diff).into_string()
    }

    #[test]
    fn format_no_change_visible() {
        let diff = FieldDiff {
            name: "Room",
            from: "101".into(),
            to: "101".into(),
            visibility: FieldVisibility::Always,
        };
        assert_eq!(render(diff), "Room: 101");
    }

    #[test]
    fn format_no_change_hidden_when_changed_visibility() {
        let diff = FieldDiff {
            name: "Status",
            from: "Regular".into(),
            to: "Regular".into(),
            visibility: FieldVisibility::Changed,
        };
        assert_eq!(render(diff), "");
    }

    #[test]
    fn format_added() {
        let diff = FieldDiff {
            name: "Activity Type",
            from: String::new(),
            to: "Exam".into(),
            visibility: FieldVisibility::Changed,
        };
        assert_eq!(render(diff), "Added Activity Type: `Exam`");
    }

    #[test]
    fn format_changed() {
        let diff = FieldDiff {
            name: "Room",
            from: "101".into(),
            to: "202".into(),
            visibility: FieldVisibility::Changed,
        };
        assert_eq!(render(diff), format!("Room: `101`{CHANGES_SEPARATOR}`202`"));
    }

    #[test]
    fn format_removed() {
        let diff = FieldDiff {
            name: "Teacher",
            from: "Smith".into(),
            to: String::new(),
            visibility: FieldVisibility::Changed,
        };
        // MarkdownV2 uses single tilde for strikethrough
        assert_eq!(render(diff), "Teacher: ~`Smith`~");
    }

    #[test]
    fn format_both_empty() {
        let diff = FieldDiff {
            name: "Additional Info",
            from: String::new(),
            to: String::new(),
            visibility: FieldVisibility::Always,
        };
        assert_eq!(render(diff), "");
    }

    #[test]
    fn field_renders_name_and_code_value() {
        let field = Field {
            name: "Subject",
            value: "Math".to_string(),
        };
        assert_eq!(render_public(field).as_str(), "Subject: `Math`");
    }
}
