use std::thread;

use crate::models::NewLogEntry;
use crate::{Uuid, diesel_impl::global_pool};
use once_cell::sync::OnceCell;
use tokio::sync::mpsc::{self, Sender};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{layer::Context, layer::Layer};

static SENDER: OnceCell<Sender<NewLogEntry>> = OnceCell::new();
static WORKER_STARTED: OnceCell<()> = OnceCell::new();

/// Initialize background worker for DB log sink if not already started.
fn ensure_worker_started() {
    if WORKER_STARTED.get().is_some() {
        return;
    }

    let (tx, mut rx) = mpsc::channel::<NewLogEntry>(1024);
    SENDER.set(tx).unwrap();

    // Spawn a blocking thread to perform Diesel writes without tying up async runtime.
    thread::spawn(move || {
        // Acquire pool once; if it fails, retry on next loop.
        loop {
            let pool = match global_pool() {
                Ok(p) => p.clone(),
                Err(_) => {
                    // Sleep a bit and retry until DB is initialized
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    continue;
                }
            };

            let mut pending: Vec<NewLogEntry> = Vec::with_capacity(256);
            // Inner loop: drain channel with short waits and batch insert
            loop {
                // Block for first item
                let first = match rx.blocking_recv() {
                    Some(v) => v,
                    None => return, // channel closed
                };
                pending.push(first);

                // Drain up to batch size with short timeout
                let start = std::time::Instant::now();
                while pending.len() < 256 && start.elapsed() < std::time::Duration::from_millis(50)
                {
                    match rx.try_recv() {
                        Ok(v) => pending.push(v),
                        Err(_) => break,
                    }
                }

                // Write batch
                if let Ok(mut conn) = pool.get() {
                    use crate::schema::logs;
                    use diesel::prelude::*;

                    let _ = diesel::insert_into(logs::table)
                        .values(&pending)
                        .execute(&mut conn);
                }
                pending.clear();
            }
        }
    });

    WORKER_STARTED.set(()).unwrap();
}

#[derive(Default)]
/// Tracing layer that forwards events into the `logs` table via a background worker.
pub struct DbLogLayer {}

/// Explicitly start the background logging worker (safe to call multiple times).
pub fn start_db_log_worker() {
    ensure_worker_started();
}

struct FieldVisitor {
    message: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            message: None,
            fields: serde_json::Map::new(),
        }
    }
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let key = field.name();
        if key == "message" {
            self.message = Some(format!("{:?}", value));
        } else {
            self.fields.insert(
                key.to_string(),
                serde_json::Value::String(format!("{:?}", value)),
            );
        }
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let key = field.name();
        if key == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }
}

impl<S> Layer<S> for DbLogLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);

        let level: String = match *meta.level() {
            Level::TRACE => "TRACE",
            Level::DEBUG => "DEBUG",
            Level::INFO => "INFO",
            Level::WARN => "WARN",
            Level::ERROR => "ERROR",
        }
        .to_string();

        let target = Some(meta.target().to_string());
        let message = visitor.message.unwrap_or_default();
        let fields = if visitor.fields.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(visitor.fields))
        };
        let file = meta.file().map(|s| s.to_string());
        let line = meta.line().map(|l| l as i32);

        let entry = NewLogEntry {
            id: Uuid::new_v4(),
            level,
            target,
            message,
            fields,
            file,
            line,
        };

        if let Some(tx) = SENDER.get() {
            let _ = tx.try_send(entry);
        }
    }
}

/// Helper to create a boxed layer to avoid type issues at call site.
pub fn db_log_layer() -> DbLogLayer {
    DbLogLayer::default()
}
