use crate::datetime::{Date, Time};
use db::{models::LessonCode, utils::AsTrimmedStr as _};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::Debug,
};
use strum::{Display, FromRepr};

/// The different types of elements that exist in the Untis API.
///
/// Serialized/deserialized as plain numbers to match the upstream API.
#[derive(
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    Serialize,
    Deserialize,
    Display,
    FromRepr,
)]
#[repr(u8)]
#[serde(from = "u8", into = "u8")]
#[strum(serialize_all = "PascalCase")]
pub enum ElementType {
    /// Unknown/unsupported type (fallback for forward-compatibility)
    #[strum(serialize = "Unknown")]
    Unknown = 0,
    Class = 1,
    Teacher = 2,
    Subject = 3,
    Room = 4,
    Student = 5,
}

impl From<u8> for ElementType {
    fn from(value: u8) -> Self {
        Self::from_repr(value).unwrap_or(ElementType::Unknown)
    }
}

impl From<ElementType> for u8 {
    fn from(value: ElementType) -> Self {
        value as u8
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Serialize, Deserialize)]
pub(crate) struct SchoolSearchResult {
    pub size: usize,
    pub schools: Vec<School>,
}

/// A school that uses Untis.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct School {
    /// The Untis instance that this school uses, e.g. `ajax.webuntis.com`
    pub server: String,

    pub use_mobile_service_url_android: bool,

    /// The school's address.
    pub address: String,

    /// The school's full name.
    pub display_name: String,

    /// A unique, shorter name for this school.
    pub login_name: String,

    /// This school's unique id in Untis.
    #[serde(rename = "schoolId")]
    pub id: usize,

    pub use_mobile_service_url_ios: bool,

    /// URL of the WebUntis login page for this school.
    pub server_url: String,

    pub mobile_service_url: Option<String>,
}

/// A Untis session.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// The session's id.
    pub session_id: String,

    /// Id of the user's class.
    #[serde(rename = "klasseId")]
    pub class_id: usize,

    /// The user's id.
    pub person_id: usize,

    /// The user's element type (Teacher or Student).
    pub person_type: ElementType,
}

/// A set of colors that can be used to display a timetable.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct StatusData {
    /// Color information for different lesson types.
    pub lstypes: Vec<HashMap<String, StatusDataItem>>,

    /// Color information for lesson statuses.
    pub codes: Vec<HashMap<String, StatusDataItem>>,
}

/// Color information to display a specific lesson.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDataItem {
    /// Foreground color, formatted as `RRGGBB`.
    pub fore_color: String,

    /// Background color, formatted as `RRGGBB`.
    pub back_color: String,
}

/// A schoolyear.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schoolyear {
    /// The schoolyears's id, unique within this school.
    pub id: usize,

    /// The schoolyear's name.
    pub name: String,

    /// The schoolyear's start date.
    pub start_date: Date,

    /// The schoolyear's end date.
    pub end_date: Date,
}

/// A school holiday.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Holiday {
    /// The holiday's id, unique within this school.
    pub id: usize,

    /// The holiday's shortened name, presumably unique within this school.
    pub name: String,

    /// The holiday's full name.
    pub long_name: String,

    /// The holiday's start date.
    pub start_date: Date,

    /// The holiday's end date.
    pub end_date: Date,
}

/// Represents a room for school lessons.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Room {
    /// The room's id, unique within this school.
    pub id: usize,

    /// The room's shortened name, unique within this school.
    pub name: String,

    /// The room's full name.
    pub long_name: String,

    /// Whether the room is generally available or not used in the system.
    pub active: bool,

    /// Foreground color for displaying the room, formatted as `RRGGBB`.
    pub fore_color: Option<String>,

    /// Background color for displaying the room, formatted as `RRGGBB`.
    pub back_color: Option<String>,

    /// The building that this room is located in. May be an empty string if you school hasn't configured any.
    pub building: String,

    pub did: Option<usize>,
}

/// Represents a school class.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Class {
    /// The class's id, unique within this school.
    pub id: usize,

    /// The class's shortened name, unique within this school.
    pub name: String,

    /// The class's full name.
    pub long_name: String,

    /// Whether the class is generally available or not used in the system.
    pub active: bool,

    /// Foreground color for displaying the class, formatted as `RRGGBB`.
    pub fore_color: Option<String>,

    /// Background color for displaying the class, formatted as `RRGGBB`.
    pub back_color: Option<String>,

    pub did: Option<usize>,

    /// Id of the class's primary teacher. May be -1 if there is none.
    #[serde(default = "default_id")]
    pub teacher1: isize,

    /// Id of the class's secondary teacher. May be -1 if there is none.
    #[serde(default = "default_id")]
    pub teacher2: isize,
}

/// Represents a school subject.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    /// The subject's id, unique within this school.
    pub id: usize,

    /// The subject's shortened name, unique within this school.
    pub name: String,

    /// The subject's full name.
    pub long_name: String,

    pub alternate_name: String,

    /// Whether the subject is generally available or not used in the system.
    pub active: bool,

    /// Foreground color for displaying the subject, formatted as `RRGGBB`.
    pub fore_color: Option<String>,

    /// Background color for displaying the subject, formatted as `RRGGBB`.
    pub back_color: Option<String>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherBase {
    /// The teacher's id, unique within this school.
    pub id: usize,

    /// The teacher's shortened name, unique within this school.
    pub name: String,
}

/// Represents a teacher.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[impl_tools::autoimpl(Deref, DerefMut using self.base)]
#[serde(rename_all = "camelCase")]
pub struct Teacher {
    #[serde(flatten)]
    base: TeacherBase,

    /// The teacher's first name.
    #[serde(rename = "foreName")]
    pub first_name: String,

    /// The teacher's last name.
    #[serde(rename = "longName")]
    pub last_name: String,

    /// The teacher's title.
    pub title: String,

    /// Whether the teacher is generally available or not used in the system.
    pub active: bool,

    pub dids: Vec<DidItem>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct DidItem {
    id: usize,
}

/// Represents a student.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Student {
    /// The student's id, unique within this school.
    pub id: usize,

    pub key: String,

    /// The student's shortened name, unique within this school.
    pub name: String,

    /// The student's first name.
    #[serde(rename = "foreName")]
    pub first_name: String,

    /// The student's last name.
    #[serde(rename = "longName")]
    pub last_name: String,

    /// The student's gender.
    pub gender: String,
}

/// A school lesson.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    /// The lesson's id.
    pub id: usize,

    /// The lesson's date.
    pub date: Date,

    /// The lesson's start time.
    pub start_time: Time,

    /// The lesson's end time.
    pub end_time: Time,

    /// The type of lesson.
    #[serde(rename = "lstype", default)]
    pub lesson_type: LessonType,

    /// The lesson's code.
    #[serde(default)]
    pub code: LessonCode,

    /// Unique id for this specific schedule.
    pub lsnumber: usize,

    /// Info Text for this specific lesson.
    #[serde(default)]
    pub lstext: String,

    /// Possible substitution text.
    pub subst_text: Option<String>,

    /// The classes that are part of this lesson.
    #[serde(rename = "kl")]
    pub classes: Vec<IdItem>,

    /// The subjects that are taught in this lesson.
    #[serde(rename = "su")]
    pub subjects: Vec<IdItem>,

    /// The rooms that this lesson takes place in.
    #[serde(rename = "ro")]
    pub rooms: Vec<IdItem>,

    /// The teachers which are teaching this lesson.
    #[serde(rename = "te", default)]
    pub teachers: Vec<IdItem>,

    #[serde(default)]
    pub statflags: String,

    /// The lesson's activity type.
    #[serde(default = "default_activity_type")]
    pub activity_type: String,
}

impl From<Lesson> for db::models::UnownedLesson {
    fn from(src: Lesson) -> Self {
        let subjects = src.subjects.iter().map(|i| i.name.clone()).collect();
        let teachers = src.teachers.iter().map(|i| i.name.clone()).collect();
        let rooms = src.rooms.iter().map(|i| i.name.clone()).collect();
        let classes = src.classes.iter().map(|i| i.name.clone()).collect();

        Self {
            lesson_id: src.id as i64,
            date: *src.date,
            end_time: *src.end_time,
            lesson_type: src.lesson_type.as_trimmed_json_string().unwrap(),
            start_time: *src.start_time,
            subst_text: src.subst_text,
            lesson_code: match src.code {
                LessonCode::Regular => db::models::LessonCode::Regular,
                LessonCode::Irregular => db::models::LessonCode::Irregular,
                LessonCode::Cancelled => db::models::LessonCode::Cancelled,
            },
            classes,
            rooms,
            subjects,
            teachers,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawHomework {
    pub id: usize,
    /// The id of the exact lesson this homework is associated with.
    pub lesson_id: usize,
    /// The homework's creation date.
    pub date: Date,
    /// The homework's deadline date.
    pub due_date: Date,
    pub completed: bool,
    pub remark: String,
    /// Always provided.
    pub text: String,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkLesson {
    pub id: usize,
    pub lesson_type: LessonType,
    pub subject: String,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkRecord {
    pub element_ids: Vec<usize>,
    pub homework_id: usize,
    pub teacher_id: usize,
}

/// Response for homework data requests.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeworksData {
    homeworks: Vec<RawHomework>,
    lessons: Vec<HomeworkLesson>,
    records: Vec<HomeworkRecord>,
    teachers: Vec<TeacherBase>,
}

impl HomeworksData {
    #[tracing::instrument(skip(self))]
    pub fn into_homeworks(self) -> Vec<Homework> {
        use std::collections::HashMap;
        // Index raw homeworks by id for fast lookup
        let mut hw_map: HashMap<usize, RawHomework> = HashMap::with_capacity(self.homeworks.len());
        for hw in self.homeworks {
            hw_map.insert(hw.id, hw);
        }

        // Index lessons by id
        let mut lesson_map: HashMap<usize, HomeworkLesson> =
            HashMap::with_capacity(self.lessons.len());
        for lesson in self.lessons {
            lesson_map.insert(lesson.id, lesson);
        }

        // Index teachers by id
        let mut teacher_map: HashMap<usize, TeacherBase> =
            HashMap::with_capacity(self.teachers.len());
        for teacher in self.teachers {
            teacher_map.insert(teacher.id, teacher);
        }

        let mut result = Vec::with_capacity(self.records.len());
        for record in self.records {
            // Find the raw homework
            let raw = match hw_map.get(&record.homework_id) {
                Some(h) => h,
                None => {
                    tracing::error!(
                        homework_id = record.homework_id,
                        homeworks = ?hw_map,
                        "Homework record references missing homework"
                    );
                    continue;
                }
            };

            // Find the lesson corresponding to the raw homework
            let lesson = match lesson_map.get(&raw.lesson_id) {
                Some(l) => l,
                None => {
                    tracing::error!(
                        lesson_id = raw.lesson_id,
                        lessons = ?lesson_map,
                        homeworks = ?hw_map,
                        "Homework references missing lesson"
                    );
                    continue;
                }
            };

            // Find the teacher referenced in the record
            let teacher = match teacher_map.get(&record.teacher_id) {
                Some(t) => t,
                None => {
                    tracing::warn!(
                        teacher_id = record.teacher_id,
                        teachers = ?teacher_map,
                        homeworks = ?hw_map,
                        "Homework record references missing teacher"
                    );
                    &TeacherBase {
                        id: 0,
                        name: String::from("N/A"),
                    }
                }
            };

            // Construct final Homework value (clone small structs)
            result.push(Homework {
                id: raw.id,
                date: raw.date,
                due_date: raw.due_date,
                is_completed: raw.completed,
                remark: raw.remark.clone(),
                text: raw.text.clone(),
                lesson: lesson.clone(),
                teacher: teacher.clone(),
            });
        }

        result
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Homework {
    pub id: usize,

    pub remark: String,
    pub text: String,
    pub lesson: HomeworkLesson,

    pub date: Date,
    pub due_date: Date,

    pub is_completed: bool,
    pub teacher: TeacherBase,
}

/// Represents the type of lesson.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug, Serialize, Deserialize,
)]
pub enum LessonType {
    #[default]
    #[serde(rename = "Unterricht")]
    Lesson,
    #[serde(rename = "oh")]
    OfficeHour,
    #[serde(rename = "sb")]
    Standby,
    #[serde(rename = "bs")]
    BreakSupervision,
    #[serde(rename = "ex")]
    Exam,
}

/// Represents an element that is part of a lesson.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct IdItem {
    /// The element's id.
    pub id: isize,

    /// The element's short name.
    pub name: String,

    /// If this element is a substitute, this is the id of the original element.
    #[serde(rename = "orgid")]
    pub orig_id: Option<isize>,

    /// If this element is a substitute, this is the name of the original element.
    #[serde(rename = "orgname")]
    pub orig_name: Option<String>,
}

impl IdItem {
    const DEFAULT_ID: isize = -1;

    pub fn has_id(&self) -> bool {
        self.id != Self::DEFAULT_ID
    }

    pub const fn from_name(name: String) -> Self {
        IdItem {
            id: Self::DEFAULT_ID,
            name,
            orig_id: None,
            orig_name: None,
        }
    }
}

impl Default for IdItem {
    fn default() -> Self {
        tracing::warn!("Using default IdItem");
        IdItem {
            id: Self::DEFAULT_ID,
            name: String::new(),
            orig_id: None,
            orig_name: None,
        }
    }
}

pub trait AsItemName {
    type Output;

    fn as_name(&self) -> Self::Output;
}

impl AsItemName for IdItem {
    type Output = String;

    fn as_name(&self) -> Self::Output {
        self.name.clone()
    }
}

impl<T: AsItemName> AsItemName for Vec<T> {
    type Output = Vec<T::Output>;

    fn as_name(&self) -> Self::Output {
        self.iter().map(AsItemName::as_name).collect::<Vec<_>>()
    }
}

/// Represents a school department.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Department {
    /// The department's id, unique within this school.
    pub id: usize,

    /// The department's short name, unique within this school.
    pub name: String,

    /// The department's full name.
    pub long_name: String,
}

fn default_id() -> isize {
    -1
}

fn default_activity_type() -> String {
    String::from("undefined")
}
