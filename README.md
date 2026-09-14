# Untisgram

<img width="234" height="337" alt="Example Untis notification in Telegram" src="https://github.com/user-attachments/assets/53fd091c-7a75-4320-bc2e-89a11edd3271" />

A Telegram bot that watches WebUntis timetables, so you don't have to open the app every morning to see if your first lesson is cancelled.

WebUntis shows changes, but only when you look. Untisgram looks for you: once a minute it loads the timetable, compares it with the previous one, and posts every change to a Telegram chat. A class group or a forum topic works just as well as a private chat, so the whole class gets the news at the same time.

## Features

- **Change notifications.** Cancelled lessons, room and teacher swaps, moved lessons, new lessons, and substitution notes. The timetable is checked about two weeks ahead.
- **Live status message.** One message that the bot keeps editing, so you can pin it: the current or next lesson, open homework sorted by deadline, and when the data was last refreshed.
- **Many timetables, one bot.** Each task watches one class or one personal timetable and can post to its own chats and forum topics.
- **No restarts for new tasks.** Add or remove tasks in the database while the bot runs. It picks them up within seconds.
- **Keeps running.** A task that fails, for example because WebUntis is down, restarts on its own. Logs are written to PostgreSQL.

## What a notification looks like

```text
Date: 2026-09-15 Tuesday
Changes in lesson:
Subject: Math
Time: 08:00 - 08:45
Teacher: MUE
Room: 101 → 204
Status: regular
```

Changed values are shown as `old → new`. A value that disappeared is crossed out.

## How it works

1. You add a task to the `bot_states` table: WebUntis school and login, the class to watch, and the Telegram chats to post to.
2. The bot logs in to WebUntis and loads the timetable once a minute.
3. It compares the new timetable with the one saved in PostgreSQL and sends a message if something changed.
4. It updates the status message and saves the new timetable for the next check.

Untisgram is written in Rust with [teloxide](https://github.com/teloxide/teloxide), [Diesel](https://diesel.rs/), and [Tokio](https://tokio.rs/). It talks to WebUntis through its JSON-RPC and REST APIs.

## Get started

- [Self-hosting](SELF_HOSTING.md): run your own instance, with or without Dokku.
- [Contributing](CONTRIBUTING.md): set up a development environment and send changes.

## Disclaimer

Untisgram is not affiliated with or endorsed by Untis GmbH. Use it only with accounts you are allowed to use, and check your school's rules before sharing timetable data in group chats.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
