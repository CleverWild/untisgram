use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use eyre::Result;
use once_cell::sync::OnceCell;

/// Diesel connection pool type (crate-private).
pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

static DB_POOL: OnceCell<DbPool> = OnceCell::new();

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Establish a connection pool for the given database URL.
/// Returns a `DbPool` wrapped in `eyre::Result` on success.
pub fn establish_pool(database_url: &str) -> Result<DbPool> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder().build(manager)?;
    Ok(pool)
}

/// Initialize pool from environment variable `DATABASE_URL` (helper).
pub fn establish_pool_from_env() -> Result<DbPool> {
    let url = std::env::var("DATABASE_URL")?;
    establish_pool(&url)
}

/// Initialize the global pool from the environment and store it in a OnceCell.
/// Calling this multiple times will return an error if already initialized.
pub fn init_global_pool_from_env() -> Result<&'static DbPool> {
    // load .env if present
    let _ = dotenvy::dotenv();
    let pool = establish_pool_from_env()?;
    // Run pending migrations once on a fresh pooled connection
    {
        let mut conn = pool.get()?;
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| eyre::eyre!("migrations failed: {e}"))?;
    }
    DB_POOL
        .set(pool)
        .map_err(|_| eyre::eyre!("database pool already initialized"))?;
    Ok(DB_POOL.get().expect("pool just set"))
}

/// Get a reference to the global pool previously initialized with
/// `init_global_pool_from_env`.
pub fn global_pool() -> Result<&'static DbPool> {
    DB_POOL
        .get()
        .ok_or_else(|| eyre::eyre!("database pool is not initialized"))
}
