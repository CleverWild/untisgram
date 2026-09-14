use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use teloxide::Bot;
use tokio::task::JoinSet;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    Layer as _, prelude::__tracing_subscriber_SubscriberExt as _, util::SubscriberInitExt as _,
};
use untisgram::{
    IS_PROD, cleanup,
    work::{WorkerContext, working_loop},
};

#[tokio::main]
async fn main() {
    // Build a single subscriber graph and initialize ONCE to avoid double-init panics.
    let env_filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        .add_directive(
            if IS_PROD {
                "untisgram=info"
            } else {
                "untisgram=trace"
            }
            .parse()
            .unwrap(),
        );

    if IS_PROD {
        // Console layer (no ANSI in prod by default)
        let console_layer = tracing_subscriber::fmt::layer()
            .with_line_number(true)
            .with_ansi(false);

        // DB log layer should capture all levels including TRACE
        let db_layer = db::logging::db_log_layer().with_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(LevelFilter::TRACE.into())
                .from_env_lossy(),
        );

        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .with(db_layer)
            .init();
    } else {
        // Pretty, colored console output for development
        let console_layer = tracing_subscriber::fmt::layer()
            .pretty()
            .with_line_number(true)
            .with_ansi(true);

        // DB log layer should capture all levels including TRACE
        let db_layer = db::logging::db_log_layer().with_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(LevelFilter::TRACE.into())
                .from_env_lossy(),
        );

        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .with(db_layer)
            .init();
    }

    let token = {
        // Try to load from environment variable first, then from file
        std::env::var("BOT_TOKEN").unwrap_or_else(|_| {
            let config = config::Config::builder()
                .add_source(config::File::with_name("Secrets.toml"))
                .build()
                .expect("Failed to load Secrets.toml or BOT_TOKEN env var");
            config
                .get::<String>("bot_token")
                .expect("Set BOT_TOKEN env var or bot_token in Secrets.toml")
        })
    };

    let bot = Bot::new(token);

    tracing::info!("Initializing database...");
    if let Err(e) = db::init_db() {
        tracing::error!("Failed to initialize database: {e}");
        panic!("Cannot continue without database");
    }

    db::models::delete_logs_before(chrono::Utc::now()).unwrap();
    tracing::info!("Database initialized");

    // Late start DB log worker after successful DB initialization
    db::logging::start_db_log_worker();

    let mut join_handles = JoinSet::new();
    let mut running_tasks: HashMap<db::Uuid, tokio::task::AbortHandle> = HashMap::new();

    // Spawn cleanup task for old lessons
    tokio::spawn(cleanup::daily_cleanup_loop());

    tracing::info!("Starting observer loop...");
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tracing::debug!("Fetching bot states...");

        // Fetch all bot states
        let bot_states = match db::models::BotTask::get_all() {
            Ok(states) => states,
            Err(e) => {
                tracing::error!("Failed to fetch bot states: {e}");
                continue;
            }
        };

        tracing::debug!("Found {} bot state(s) in database", bot_states.len());

        // Check for new or missing states
        let current_state_ids: HashSet<_> = bot_states.iter().map(|s| s.id).collect();

        // Remove tasks for states that no longer exist
        running_tasks.retain(|id, abort_handle| {
            if !current_state_ids.contains(id) {
                tracing::warn!("Bot state {id} no longer exists, stopping worker");
                abort_handle.abort();
                false
            } else {
                true
            }
        });

        // Start workers for new states
        for task in bot_states {
            if running_tasks.contains_key(&task.id) {
                continue; // Already running
            }

            tracing::info!("Starting worker for task: {:?}", task);

            let school = match webuntis::schools::get_by_name(task.untis_school.as_str()).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(
                        "Failed to get school '{}' for task {}: {e}",
                        task.untis_school,
                        task.task_name
                    );
                    continue;
                }
            };

            let untis_client = match school
                .client_login(task.untis_login.as_str(), task.untis_password.as_str())
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to login to Untis for task {}: {e}", task.task_name);
                    continue;
                }
            };

            // Wrap the future to return identifying info along with the result
            let state_id = task.id;
            let task_name = task.task_name.clone();

            let ctx = WorkerContext {
                bot: bot.clone(),
                task,
                untis_client,
                engaged_at: chrono::Utc::now(),
            };

            let abort_handle =
                join_handles.spawn(async move { (state_id, task_name, working_loop(ctx).await) });
            running_tasks.insert(state_id, abort_handle);
        }

        loop {
            tokio::select! {
                _ = interval.tick() => break,
                Some(result) = join_handles.join_next() => {
                    match result {
                        // Task returned an application error
                        Ok((state_id, task_name, Err(e))) => {
                            running_tasks.remove(&state_id);
                            tracing::error!(
                                task = %task_name,
                                error = %e,
                                "Worker encountered an error, will restart on next cycle"
                            );
                            interval.reset_after(Duration::from_secs(1));
                        }
                        // Join error: cancelled or panicked
                        Err(join_err) => {
                            if join_err.is_panic() {
                                tracing::error!("Worker panicked: {join_err}");
                                interval.reset_after(Duration::from_secs(1));
                            }
                            // Cancelled tasks are expected
                        }
                    }
                }
            }
        }
    }
}
