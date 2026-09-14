# Contributing

## Set up

1. Install [mise](https://mise.jdx.dev/getting-started.html).
2. Install the project tools (Rust, taplo, etc.):

   ```shell
   mise install
   ```

3. Start PostgreSQL. The committed `.env` expects this container:

   ```shell
   docker run -d --name untisgram-db \
     -e POSTGRES_USER=untis -e POSTGRES_PASSWORD=pass -e POSTGRES_DB=untis \
     -p 127.0.0.1:5433:5432 postgres:17
   ```

4. Create `Secrets.toml` with the token of a test bot. Git ignores this file.

   ```toml
   bot_token = "123456:token"
   ```

5. In [src/lib.rs](src/lib.rs), set `DEBUG_TELEGRAM_CHAT` to your own chat ID. Do not commit this change.
6. Run the bot:

   ```shell
   cargo run
   ```

Debug builds print detailed colored logs and send notifications only to `DEBUG_TELEGRAM_CHAT`, so you can test with real WebUntis accounts without writing to their chats. To add a task, see [Add a task](SELF_HOSTING.md#5-add-a-task).

## Checks

CI runs these on every pull request. Run them before you push:

```shell
mise run fmt-check
mise run clippy
cargo test --workspace
```

`mise run fmt` fixes Rust and TOML formatting.

The ignored test `send_test_messages` sends sample messages to `DEBUG_TELEGRAM_CHAT` through the bot in `Secrets.toml`. Run it with `cargo test send_test_messages -- --ignored`.

## Project layout

- `src/`: the bot process, worker lifecycle, status message, and message formatting.
- `crates/db/`: PostgreSQL access with Diesel and the embedded migrations.
- `crates/webuntis/`: the WebUntis JSON-RPC and REST client.

## Database changes

Add a new folder in `crates/db/migrations/` with `up.sql` and `down.sql`, and update `crates/db/src/schema.rs` to match. The bot applies new migrations when it starts. [Diesel CLI](https://diesel.rs/guides/getting-started) can generate both files, but mise does not install it.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/), for example `feat(status): show homework due tomorrow` or `chore(deps): bump dependencies`.
