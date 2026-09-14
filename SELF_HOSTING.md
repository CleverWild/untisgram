# Self-hosting

## Requirements

- [mise](https://mise.jdx.dev/getting-started.html). It installs the Rust toolchain for the build.
- PostgreSQL 13 or newer.
- A Telegram bot token from [@BotFather](https://t.me/BotFather).
- A WebUntis account that can see the timetable you want to watch.

## 1. Build

```shell
git clone https://github.com/CleverWild/untisgram.git
cd untisgram
mise install
```

Before building, open [src/main.rs](src/main.rs) and set `DEBUG_TELEGRAM_CHAT` to your own Telegram chat ID. The bot sends a copy of every notification with extra debug details to that chat.

```shell
cargo build --release
```

The binary is `target/release/untisgram`. Always use a release build: debug builds send notifications only to `DEBUG_TELEGRAM_CHAT`, never to the chats you configure.

## 2. Start PostgreSQL

Skip this step if you already have a server. Otherwise, start one with Docker:

```shell
docker run -d --name untisgram-db --restart unless-stopped \
  -e POSTGRES_USER=untis -e POSTGRES_PASSWORD=pass -e POSTGRES_DB=untis \
  -p 127.0.0.1:5433:5432 postgres:17
```

## 3. Configure

| Variable       | Required | Description                                                         |
| -------------- | -------- | ------------------------------------------------------------------- |
| `DATABASE_URL` | yes      | PostgreSQL connection string.                                       |
| `BOT_TOKEN`    | yes      | Telegram bot token.                                                 |
| `RUST_LOG`     | no       | Log filter, for example `debug` or `warn`. Defaults to `info`.      |

The bot also reads these files from its working directory:

- `.env` for any variable above, for example `DATABASE_URL=postgres://untis:pass@127.0.0.1:5433/untis`.
- `Secrets.toml` with `bot_token = "123456:token"`, used when `BOT_TOKEN` is not set.

## 4. Run

```shell
DATABASE_URL=postgres://untis:pass@127.0.0.1:5433/untis BOT_TOKEN=123456:token ./target/release/untisgram
```

On start, the bot applies database migrations and clears old entries in the `logs` table. It does nothing else until you add a task.

## 5. Add a task

Every row in the `bot_states` table is one task: one WebUntis timetable and the chats that receive its updates. The bot checks the table every 10 seconds, starts new tasks, and stops deleted ones.

```sql
INSERT INTO bot_states (
    id, task_name, untis_school, untis_login, untis_password,
    target_class_name, timezone,
    notification_chat_id, notification_thread_id,
    status_chat_id, status_thread_id,
    updated_at
) VALUES (
    gen_random_uuid(), 'class-10a', 'my-school', 'student.login', 'secret',
    '10A', 'Europe/Berlin',
    -1001234567890, NULL,
    -1001234567890, NULL,
    now()
);
```

| Column | Description |
| --- | --- |
| `task_name` | Any name. It appears in logs. |
| `untis_school` | School to search for in WebUntis; the first match is used. The school name from the WebUntis login URL (`?school=...`) works best. |
| `untis_login`, `untis_password` | WebUntis account. The password is stored as plain text. |
| `target_class_name` | Class name as shown in WebUntis, for example `10A`. `NULL` watches the account's own timetable. |
| `timezone` | [IANA time zone](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones). Defaults to `Europe/Berlin`. |
| `notification_chat_id`, `notification_thread_id` | Chat for timetable changes. Set the thread ID to post into a forum topic, otherwise `NULL`. |
| `status_chat_id`, `status_thread_id` | Chat for the status message. The bot sends it once and then edits it. |
| `status_message_id` | Leave empty. The bot fills it in. |

To find a chat ID, add the bot to the chat and send a command such as `/start@your_bot` there. Then open `https://api.telegram.org/bot<token>/getUpdates` and read `message.chat.id`. In a forum topic, `message.message_thread_id` is the thread ID.

Changes to chats, class, and time zone apply within a minute. Changes to the school, login, or password apply after the bot restarts.

## Deploy with Dokku

The repository contains `Procfile`, `RustConfig`, and `.buildpacks`, so Dokku builds a release binary and starts it as a `worker` process. Remember to set `DEBUG_TELEGRAM_CHAT` first.

```shell
dokku apps:create untisgram
dokku postgres:create untisgram-db
dokku postgres:link untisgram-db untisgram
dokku config:set untisgram BOT_TOKEN=123456:token
dokku ps:scale untisgram worker=1
```

`postgres:link` needs the [dokku-postgres](https://github.com/dokku/dokku-postgres) plugin and sets `DATABASE_URL`. Push the code with `git push dokku main` after adding the remote `dokku@<your-server>:untisgram`.

The workflow in `.github/workflows/deploy.yml` deploys every push to `main`. To use it in a fork, set the `DOKKU_HOST` and `DOKKU_SSH_PRIVATE_KEY` repository secrets and change the app name in `git_remote_url`.
