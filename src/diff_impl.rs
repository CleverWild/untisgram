use chrono::NaiveDate;
use db::models::LessonCode;

#[derive(Debug, Clone)]
pub enum Diff<'a> {
    Added(&'a webuntis::Lesson),
    Changed {
        from: &'a db::models::Lesson,
        to: &'a webuntis::Lesson,
    },
}

impl Diff<'_> {
    pub fn find<'a>(
        prev_schedule: &'a [db::models::Lesson],
        new_schedule: &'a [webuntis::Lesson],
    ) -> Vec<Diff<'a>> {
        let mut diff = Vec::new();

        for new_lesson in new_schedule {
            if let Some(prev_lesson) = prev_schedule
                .iter()
                .find(|l| l.lesson_id as usize == new_lesson.id)
            {
                if !Self::lessons_equal(prev_lesson, new_lesson) {
                    diff.push(Diff::Changed {
                        from: prev_lesson,
                        to: new_lesson,
                    });
                }
            } else {
                diff.push(Diff::Added(new_lesson));
            }
        }

        diff
    }

    // Compare DB lesson with Untis lesson using UnownedLesson (fields relevant for messaging)
    fn lessons_equal(prev: &db::models::Lesson, new: &webuntis::Lesson) -> bool {
        // Date is not part of UnownedLesson; compare it explicitly to capture day changes
        if prev.date != new.date.0 {
            return false;
        }

        // Normalize order-insensitive collections before equality
        fn normalize(mut m: db::models::UnownedLesson) -> db::models::UnownedLesson {
            m.subjects.sort();
            m.teachers.sort();
            m.rooms.sort();
            m.classes.sort();
            m
        }

        let left = normalize(db::models::UnownedLesson::from(prev.to_owned()));
        let right = normalize(db::models::UnownedLesson::from(new.to_owned()));

        left == right
    }

    pub fn date(&self) -> NaiveDate {
        match self {
            Diff::Added(lesson) => lesson.date.0,
            Diff::Changed { from, .. } => from.date,
        }
    }

    pub fn start_time(&self) -> webuntis::Time {
        match self {
            Diff::Added(lesson) => lesson.start_time,
            Diff::Changed { from, .. } => webuntis::Time(from.start_time),
        }
    }

    pub fn end_time(&self) -> webuntis::Time {
        match self {
            Diff::Added(lesson) => lesson.end_time,
            Diff::Changed { from, .. } => webuntis::Time(from.end_time),
        }
    }

    pub fn code(&self) -> &LessonCode {
        match self {
            Diff::Added(lesson) => &lesson.code,
            Diff::Changed { to, .. } => &to.code,
        }
    }
}
