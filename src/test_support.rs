use std::path::PathBuf;

use chrono::{NaiveDate, NaiveTime};
use db::models::LessonCode;
use webuntis::{Date, Homework, HomeworkLesson, IdItem, Lesson, LessonType, TeacherBase, Time};

pub(super) fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

pub(super) fn id_item(id: isize, name: &str) -> IdItem {
    IdItem {
        id,
        name: name.to_string(),
        orig_id: None,
        orig_name: None,
    }
}

pub(super) fn untis_lesson(
    id: usize,
    date: NaiveDate,
    start_hour: u32,
    end_hour: u32,
    subject: &str,
    teacher: &str,
    room: &str,
) -> Lesson {
    Lesson {
        id,
        date: Date(date),
        start_time: Time(NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap()),
        end_time: Time(NaiveTime::from_hms_opt(end_hour, 0, 0).unwrap()),
        lesson_type: LessonType::Lesson,
        code: LessonCode::Regular,
        lsnumber: 1,
        lstext: String::new(),
        subst_text: None,
        classes: vec![id_item(1, "VABO1")],
        subjects: vec![id_item(10, subject)],
        rooms: vec![id_item(100, room)],
        teachers: vec![id_item(50, teacher)],
        statflags: String::new(),
        activity_type: "Unterricht".to_string(),
    }
}

pub(super) fn db_lesson(lesson: &Lesson) -> db::models::Lesson {
    db::models::Lesson {
        subjects: lesson.subjects.iter().map(|i| i.name.clone()).collect(),
        teachers: lesson.teachers.iter().map(|i| i.name.clone()).collect(),
        rooms: lesson.rooms.iter().map(|i| i.name.clone()).collect(),
        classes: lesson.classes.iter().map(|i| i.name.clone()).collect(),
        lesson_id: lesson.id as i64,
        date: lesson.date.0,
        end_time: lesson.end_time.0,
        lesson_code: lesson.code,
        lesson_type: serde_json::to_string(&lesson.lesson_type)
            .unwrap()
            .trim_matches('"')
            .to_string(),
        start_time: lesson.start_time.0,
        subst_text: lesson.subst_text.clone(),
        bot_state: db::Uuid::nil(),
    }
}

pub(super) fn homework(
    id: usize,
    subject: &str,
    teacher: &str,
    text: &str,
    created: NaiveDate,
    due: NaiveDate,
) -> Homework {
    Homework {
        id,
        remark: String::new(),
        text: text.to_string(),
        lesson: HomeworkLesson {
            id,
            lesson_type: LessonType::Lesson,
            subject: subject.to_string(),
        },
        date: Date(created),
        due_date: Date(due),
        is_completed: false,
        teacher: TeacherBase {
            id,
            name: teacher.to_string(),
        },
    }
}

/// One variant per file in `tests/golden/`. A variant no test uses fails compilation, and a
/// file without a variant fails `every_golden_file_has_a_variant`. Not `pub`: rustc skips the
/// dead-code check for `pub` items of a library.
#[deny(dead_code)]
#[derive(Clone, Copy, strum::IntoStaticStr, strum::VariantNames)]
#[strum(serialize_all = "snake_case")]
pub(super) enum Golden {
    EscapingDiagnostic,
    EscapingPublic,
    LessonAdded,
    LessonCancelled,
    LessonChanged,
    LessonMultipleSubjects,
    NotificationDiagnostic,
    NotificationPublic,
    StatusCurrentLesson,
    StatusMultipleHomework,
    StatusNextLesson,
    StatusNoHomework,
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

pub(super) fn assert_golden(golden: Golden, actual: &str) {
    let name: &'static str = golden.into();
    let path = golden_dir().join(format!("{name}.txt"));
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read golden file {}: {error}", path.display()));
    assert_eq!(actual, expected, "output differs from {}", path.display());
}

#[test]
fn every_golden_file_has_a_variant() {
    use strum::VariantNames as _;

    let orphans: Vec<String> = std::fs::read_dir(golden_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|file| {
            file.strip_suffix(".txt")
                .is_none_or(|name| !Golden::VARIANTS.contains(&name))
        })
        .collect();
    assert!(
        orphans.is_empty(),
        "tests/golden has files without a Golden variant: {orphans:?}"
    );
}
