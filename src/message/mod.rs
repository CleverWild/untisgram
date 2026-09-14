//! Telegram MarkdownV2 messages built from composable [`Fragment`] parts.

mod event;
mod parts;
mod render;
mod stream;

pub use parts::{
    Bold, Code, CodeBlock, DebugBlock, Each, Either, Italic, Line, Link, Newline, Separator,
    Spoiler, Strike, Text,
};
pub use render::{Diagnostic, Public, Rendered, RenderedPair, render_both, render_public};
pub use stream::{Fragment, Render};

#[cfg(test)]
mod tests {
    use teloxide::Bot;

    use super::*;
    use crate::{
        DEBUG_TELEGRAM_CHAT,
        diff_impl::Diff,
        message_formatter::format_message,
        test_support::{date, db_lesson, untis_lesson},
        utils::send_or_edit_message,
        work::Chat,
    };

    #[tokio::test]
    #[ignore]
    async fn send_test_messages() {
        let token = {
            let config = config::Config::builder()
                .add_source(config::File::with_name("Secrets.toml"))
                .build()
                .expect("Failed to load Secrets.toml");
            config
                .get::<String>("bot_token")
                .expect("Set TELOXIDE_TOKEN or TELEGRAM_BOT_TOKEN to run this test")
        };

        let bot = Bot::new(token);

        let from_lesson = db_lesson(&untis_lesson(
            12345,
            date(2025, 9, 18),
            10,
            11,
            "Mathematics",
            "Smith",
            "101",
        ));
        let to_lesson = untis_lesson(
            12345,
            date(2025, 9, 18),
            10,
            11,
            "Mathematics",
            "Johnson", // Teacher changed
            "202",     // Room changed
        );

        let samples = [
            // Only normal text
            render_both((
                Line(Text("Changes in timetable:")),
                Line(Text("Date: 2025-09-16 Tuesday")),
                Line(Text("Subject: Math (Room 101)")),
                Line(CodeBlock {
                    code: "123123",
                    language: None,
                }),
            )),
            // Normal + debug
            render_both((
                Line(Text("Changes in timetable:")),
                Line(Text("Room change: 101 → 202")),
                Newline,
                DebugBlock(Text(
                    "Debug: lesson_id=12345 original_room=101 new_room=202",
                )),
            )),
            // Multiple dates separated
            render_both((
                Newline,
                Line(Text("Date: 2025-09-16 Tuesday")),
                Line(Text("Physics → Chemistry (Lab)")),
                Newline,
                DebugBlock(Text("Debug: diff_type=SubjectSwap id=777")),
                Separator,
                Newline,
                Line(Text("Date: 2025-09-17 Wednesday")),
                Line(Text("Added lesson: Biology Extra Session")),
                Newline,
                DebugBlock(Text("Debug: new_lesson_id=888 kind=Added")),
            )),
            // Changed lesson (from/to comparison)
            render_both(format_message(Diff::Changed {
                from: &from_lesson,
                to: &to_lesson,
            })),
        ];

        let preview_chat = Chat::public(DEBUG_TELEGRAM_CHAT.id, DEBUG_TELEGRAM_CHAT.thread_id);
        for (i, sample) in samples.into_iter().enumerate() {
            println!("===== Sending test message #{} =====", i + 1);
            if let Err(e) = send_or_edit_message(&bot, preview_chat, sample.public, &mut None).await
            {
                panic!("Failed to send public test message #{i}: {e:?}");
            }
            if let Err(e) =
                send_or_edit_message(&bot, DEBUG_TELEGRAM_CHAT, sample.diagnostic, &mut None).await
            {
                panic!("Failed to send diagnostic test message #{i}: {e:?}");
            }
        }
    }
}
