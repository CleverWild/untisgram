//! Database cleanup tasks

use chrono::TimeZone;
use std::convert::Infallible;

/// Run daily cleanup task to remove past lessons from the database
#[tracing::instrument(skip_all)]
pub async fn daily_cleanup_loop() -> Result<Infallible, eyre::Report> {
    const ONE_DAY: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

    // Calculate time until next midnight (local time)
    let (next_midnight, next_run_time) = {
        let now = chrono::Local::now();
        let tomorrow = now.date_naive() + chrono::Duration::days(1);
        let midnight_naive = tomorrow.and_hms_opt(0, 0, 0).unwrap();
        let midnight_dt = chrono::Local
            .from_local_datetime(&midnight_naive)
            .single()
            .unwrap_or_else(chrono::Local::now);
        let duration_until = (midnight_dt - now).to_std().unwrap_or(ONE_DAY);
        (tokio::time::Instant::now() + duration_until, midnight_dt)
    };

    tracing::info!(
        "Daily cleanup task scheduled for: {}",
        next_run_time.format("%Y-%m-%d %H:%M:%S")
    );

    let mut interval = tokio::time::interval_at(next_midnight, ONE_DAY);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        tracing::info!("Running daily cleanup of old lessons...");

        match cleanup_old_lessons().await {
            Ok(deleted_count) => {
                tracing::info!("Cleaned up {deleted_count} old lesson(s) from database");
            }
            Err(e) => {
                tracing::error!("Failed to cleanup old lessons: {e}");
            }
        }
    }
}

/// Remove lessons that have already occurred (date is in the past)
async fn cleanup_old_lessons() -> eyre::Result<usize> {
    let today = chrono::Local::now().date_naive();

    let deleted_count = db::models::delete_lessons_before(today)?;

    Ok(deleted_count)
}
