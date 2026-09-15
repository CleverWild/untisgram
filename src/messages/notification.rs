use chrono::NaiveDate;

use crate::{
    diff_impl::Diff,
    message::{
        Bold, Each, Fragment, Line, MAX_TELEGRAM_TEXT_CHARS, Newline, Render, RenderedPair, Text,
        render_both,
    },
    messages::{
        DATE_FORMAT,
        lesson::{DiagnosticInfo, LessonMessage, lesson_message},
    },
};

#[derive(Debug, thiserror::Error)]
#[error("lesson {lesson_id} renders to {actual} characters, exceeding Telegram limit {limit}")]
pub(crate) struct NotificationTooLong {
    lesson_id: i64,
    actual: usize,
    limit: usize,
}

/// Renders the lesson changes as Telegram messages, one date per message. Public and diagnostic
/// variants share page boundaries.
pub(crate) fn render_notification_pages(
    diffs: Vec<Diff<'_>>,
) -> Result<Vec<RenderedPair>, NotificationTooLong> {
    render_notification_pages_with_limit(diffs, MAX_TELEGRAM_TEXT_CHARS)
}

fn render_notification_pages_with_limit(
    diffs: Vec<Diff<'_>>,
    limit: usize,
) -> Result<Vec<RenderedPair>, NotificationTooLong> {
    let mut entries: Vec<LessonMessage> = diffs.into_iter().map(lesson_message).collect();
    // A changed lesson is listed under its new date, which can differ from the sort key of `Diff`.
    entries.sort_by_key(|entry| entry.lesson.date);

    let mut pages = Vec::new();
    for same_date in entries.chunk_by(|a, b| a.lesson.date == b.lesson.date) {
        paginate_date(same_date, limit, &mut pages)?;
    }
    Ok(pages)
}

/// Splits one date's entries into pages that fit `limit`, never cutting an entry apart.
fn paginate_date(
    entries: &[LessonMessage],
    limit: usize,
    pages: &mut Vec<RenderedPair>,
) -> Result<(), NotificationTooLong> {
    let Some(first) = entries.first() else {
        return Ok(());
    };
    let mut page = NotificationPage {
        date: first.lesson.date,
        entries: Vec::new(),
    };
    let mut accepted = None;

    for entry in entries {
        page.entries.push(entry.clone());
        let mut candidate = render_both(page.clone());

        if candidate.max_char_count() > limit && page.entries.len() > 1 {
            pages.extend(accepted.take());
            let previous = page.entries.len() - 1;
            page.entries.drain(..previous);
            candidate = render_both(page.clone());
        }

        let actual = candidate.max_char_count();
        if actual > limit {
            return Err(NotificationTooLong {
                lesson_id: entry.lesson.lesson_id,
                actual,
                limit,
            });
        }
        accepted = Some(candidate);
    }

    pages.extend(accepted);
    Ok(())
}

#[derive(Clone)]
struct NotificationPage {
    date: NaiveDate,
    entries: Vec<LessonMessage>,
}

impl Fragment for NotificationPage {
    fn parts(self) -> impl Render {
        let entries = self.entries.into_iter().map(|lesson| {
            let diagnostic = DiagnosticInfo::from(&lesson);
            (Newline, lesson, diagnostic)
        });

        (
            Line(Bold(Text(self.date.format(DATE_FORMAT).to_string()))),
            Each(entries),
        )
            .parts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Golden, assert_golden, date, db_lesson, untis_lesson};

    const MESSAGE_BREAK: &str = "\n<<< next message >>>\n";

    struct Fixture {
        added_first_day: webuntis::Lesson,
        changed_from: db::models::Lesson,
        changed_to: webuntis::Lesson,
        added_second_day: webuntis::Lesson,
    }

    fn fixture() -> Fixture {
        let original = untis_lesson(
            102,
            date(2025, 9, 16),
            10,
            11,
            "Mathematics",
            "Smith",
            "R.101",
        );
        let mut changed_to = untis_lesson(
            102,
            date(2025, 9, 16),
            10,
            11,
            "Mathematics",
            "Johnson",
            "R.202",
        );
        changed_to.subst_text = Some("Room swap (see board)".to_string());
        let mut added_second_day = untis_lesson(
            103,
            date(2025, 9, 17),
            12,
            13,
            "Chemistry",
            "Klein",
            "Lab-1",
        );
        added_second_day.code = db::models::LessonCode::Cancelled;

        Fixture {
            added_first_day: untis_lesson(101, date(2025, 9, 16), 8, 9, "Biology", "Weber", "B.2"),
            changed_from: db_lesson(&original),
            changed_to,
            added_second_day,
        }
    }

    fn diffs(fixture: &Fixture) -> Vec<Diff<'_>> {
        vec![
            Diff::Added(&fixture.added_first_day),
            Diff::Changed {
                from: &fixture.changed_from,
                to: &fixture.changed_to,
            },
            Diff::Added(&fixture.added_second_day),
        ]
    }

    #[test]
    fn golden_notification() {
        let fixture = fixture();
        let pages = render_notification_pages(diffs(&fixture)).unwrap();
        assert_eq!(pages.len(), 2);

        let public: Vec<&str> = pages.iter().map(|page| page.public.as_str()).collect();
        let diagnostic: Vec<&str> = pages.iter().map(|page| page.diagnostic.as_str()).collect();
        assert_golden(Golden::NotificationPublic, &public.join(MESSAGE_BREAK));
        assert_golden(
            Golden::NotificationDiagnostic,
            &diagnostic.join(MESSAGE_BREAK),
        );

        let diagnostic = diagnostic.concat();
        assert_eq!(diagnostic.matches("Mathematics").count(), 1);
        assert_eq!(diagnostic.matches("*Debug*").count(), 3);
        assert!(!diagnostic.contains("Lesson \\{"));
        assert!(!diagnostic.contains("Status: regularDebug"));
    }

    fn lessons_on(day: u32, count: usize) -> Vec<webuntis::Lesson> {
        (0..count)
            .map(|index| {
                let mut lesson = untis_lesson(
                    300 + index,
                    date(2025, 9, day),
                    8 + index as u32,
                    9 + index as u32,
                    "Geography",
                    "Novak",
                    "G.4",
                );
                lesson.subst_text = Some("field trip ".repeat(3));
                lesson
            })
            .collect()
    }

    fn added(lessons: &[webuntis::Lesson]) -> Vec<Diff<'_>> {
        lessons.iter().map(Diff::Added).collect()
    }

    fn page_size(lessons: &[webuntis::Lesson]) -> usize {
        render_both(NotificationPage {
            date: lessons[0].date.0,
            entries: added(lessons).into_iter().map(lesson_message).collect(),
        })
        .max_char_count()
    }

    #[test]
    fn notification_pages_accept_a_page_of_exactly_the_limit() {
        let lessons = lessons_on(16, 2);
        let limit = page_size(&lessons);

        let pages = render_notification_pages_with_limit(added(&lessons), limit).unwrap();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].max_char_count(), limit);
    }

    #[test]
    fn notification_pages_move_the_next_entry_to_a_page_with_the_same_heading() {
        let lessons = lessons_on(16, 3);
        let limit = page_size(&lessons[..2]);

        let pages = render_notification_pages_with_limit(added(&lessons), limit).unwrap();

        assert_eq!(pages.len(), 2);
        for page in &pages {
            assert!(page.public.char_count() <= limit);
            assert!(page.diagnostic.char_count() <= limit);
            assert!(page.public.as_str().starts_with("*Tue, 16 Sep*\n\n"));
            assert!(page.diagnostic.as_str().starts_with("*Tue, 16 Sep*\n\n"));
        }
        assert_eq!(pages[0].public.as_str().matches("New lesson").count(), 2);
        assert_eq!(pages[1].public.as_str().matches("New lesson").count(), 1);
        assert!(pages[1].public.as_str().contains("10:00–11:00"));
    }

    #[test]
    fn notification_pages_never_share_a_page_between_dates() {
        let mut lessons = lessons_on(16, 1);
        lessons.extend(lessons_on(17, 1));

        let pages = render_notification_pages_with_limit(added(&lessons), usize::MAX).unwrap();

        assert_eq!(pages.len(), 2);
        assert!(pages[0].public.as_str().starts_with("*Tue, 16 Sep*\n"));
        assert!(pages[1].public.as_str().starts_with("*Wed, 17 Sep*\n"));
    }

    #[test]
    fn notification_pages_group_a_changed_lesson_under_its_new_date() {
        let from = db_lesson(&untis_lesson(
            401,
            date(2025, 9, 16),
            8,
            9,
            "Art",
            "Lee",
            "A.3",
        ));
        let to = untis_lesson(401, date(2025, 9, 18), 8, 9, "Art", "Lee", "A.3");
        let mut other = lessons_on(17, 1);
        other[0].id = 402;

        let diffs = vec![
            Diff::Changed {
                from: &from,
                to: &to,
            },
            Diff::Added(&other[0]),
        ];
        let pages = render_notification_pages_with_limit(diffs, usize::MAX).unwrap();

        assert_eq!(pages.len(), 2);
        assert!(pages[0].public.as_str().starts_with("*Wed, 17 Sep*\n"));
        assert!(pages[1].public.as_str().starts_with("*Thu, 18 Sep*\n"));
    }

    #[test]
    fn notification_pages_reject_an_entry_that_cannot_fit_alone() {
        let lessons = lessons_on(16, 2);
        let limit = page_size(&lessons[..1]) - 1;

        let error = render_notification_pages_with_limit(added(&lessons), limit).unwrap_err();

        assert_eq!(error.lesson_id, 300);
        assert_eq!(error.actual, limit + 1);
        assert_eq!(error.limit, limit);
    }
}
