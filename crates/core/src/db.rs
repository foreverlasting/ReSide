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
    run_migrations(&pool).await?;
    Ok(pool)
}

/// Run pending migrations, translating the one failure mode a *released* binary
/// can hit when the user also runs a newer build against the same data dir: the
/// DB has an applied migration this binary's migrator doesn't contain. sqlx
/// reports that as `VersionMissing`; we surface it as the actionable
/// [`AppError::DatabaseTooNew`] (→ "update ReSide") instead of a generic
/// "migration error".
async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    MIGRATOR.run(pool).await.map_err(|e| match e {
        sqlx::migrate::MigrateError::VersionMissing(v) => crate::error::AppError::DatabaseTooNew(v),
        other => other.into(),
    })
}

/// Open an in-memory database with migrations applied (for tests).
pub async fn open_in_memory() -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    run_migrations(&pool).await?;
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

    /// A DB that carries a migration this build doesn't know about (i.e. it was
    /// written by a *newer* ReSide) must open as the actionable
    /// `DatabaseTooNew`, not a generic migration error. Reproduces the
    /// downgrade collision a released binary hits against a data dir a newer
    /// local build already migrated.
    #[tokio::test]
    async fn db_from_a_newer_build_reports_database_too_new() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("data.db");

        // Migrate normally, then forge an applied migration from "the future".
        let pool = open(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations \
             (version, description, installed_on, success, checksum, execution_time) \
             VALUES (9999, 'from a newer ReSide', CURRENT_TIMESTAMP, 1, ?, 0)",
        )
        .bind(vec![0u8; 48])
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        // Reopening with this (older) migrator must detect the downgrade.
        let err = open(&db).await.unwrap_err();
        assert!(
            matches!(err, crate::error::AppError::DatabaseTooNew(9999)),
            "expected DatabaseTooNew(9999), got {err:?}"
        );
    }
}
