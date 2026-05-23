//! SQLite setup + migration runner.
//!
//! WAL mode is mandatory: the UI and the background agent are separate
//! processes that both read the DB, and WAL keeps reads from blocking the
//! single writer (see plan.md §Process coordination).

use crate::error::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Open (creating if needed) the SQLite database at `path` in WAL mode and run
/// all pending migrations. The parent directory must already exist.
pub async fn open(path: impl AsRef<Path>) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.as_ref().display()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// Open an in-memory database with migrations applied (for tests).
pub async fn open_in_memory() -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_apply_to_a_fresh_on_disk_db() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("data.db");
        let pool = open(&db).await.unwrap();

        // All seven tables should exist.
        let tables: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        for expected in [
            "activity_log",
            "apple_quota_events",
            "apps",
            "devices",
            "installations",
            "jobs",
            "signing_profiles",
        ] {
            assert!(
                tables.contains(&expected.to_string()),
                "missing table: {expected}"
            );
        }

        // WAL mode is active.
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced() {
        let pool = open_in_memory().await.unwrap();
        // Inserting an installation referencing a missing app/device must fail.
        let res = sqlx::query(
            "INSERT INTO installations (app_id, device_udid, signing_method, install_ts, expiration_ts) \
             VALUES (1, 'nope', 'free', 0, 0)",
        )
        .execute(&pool)
        .await;
        assert!(res.is_err(), "foreign key violation should be rejected");
    }
}
