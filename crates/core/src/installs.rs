//! Recording a sign+install in SQLite.
//!
//! After the forked signer ([`crate::signer`]) installs an IPA, ReSide writes
//! the result across `devices`, `apps`, `signing_profiles`, and `installations`
//! so two later features can rely on it: the Dashboard lists what's installed,
//! and the refresh agent (task 11c) finds installs whose `expiration_ts` is
//! approaching and re-signs them before the 7-day free profile lapses.
//!
//! Secrets never land here. The Apple ID is stored only as a SHA-256
//! [`apple_id_hash`]; the password lives in the keyring, referenced indirectly
//! through `signing_profiles.secret_ref` (see [`crate::signer`]).

use crate::error::Result;
use crate::ipa_meta::IpaMetadata;
use crate::ipa_store::StoredIpa;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// Free Apple-ID provisioning profiles are valid for 7 days. We persist the
/// resulting expiry so the refresh agent knows the deadline to re-sign before.
pub const FREE_PROFILE_LIFETIME_SECS: i64 = 7 * 24 * 60 * 60;

/// The signing method recorded for the free Apple-ID path driven by Sideloader.
const SIGNING_METHOD_FREE: &str = "free";

/// Stable hash of an Apple ID for non-reversible persistence. Case- and
/// whitespace-insensitive so the same account always maps to the same row.
pub fn apple_id_hash(apple_id: &str) -> String {
    hex::encode(Sha256::digest(apple_id.trim().to_lowercase().as_bytes()))
}

/// Minimal device identity needed to satisfy the `installations -> devices`
/// foreign key. Pairing doesn't currently touch SQLite, so the install path
/// upserts the device row itself.
#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub udid: String,
    pub name: Option<String>,
    pub ios_version: Option<String>,
}

/// Everything needed to record one successful install.
#[derive(Debug, Clone)]
pub struct InstallRecord<'a> {
    pub device: &'a DeviceRow,
    pub meta: &'a IpaMetadata,
    pub stored_ipa: &'a StoredIpa,
    /// Plaintext Apple ID — hashed before it touches the DB.
    pub apple_id: &'a str,
    /// Team id if known (the delegated `sideloader install` doesn't report it,
    /// so this is usually `None` for now).
    pub team_id: Option<&'a str>,
    /// Unix seconds the install completed.
    pub installed_at: i64,
}

/// Identifiers of the rows written/updated by [`record_install`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedInstall {
    pub installation_id: i64,
    pub app_id: i64,
    pub expiration_ts: i64,
}

/// Record (or update, on re-install/refresh) one install in a single
/// transaction. The natural key for an installation is (app, device): signing
/// the same app again updates the existing row's timestamps rather than
/// accumulating duplicates.
pub async fn record_install(pool: &SqlitePool, rec: &InstallRecord<'_>) -> Result<RecordedInstall> {
    let id_hash = apple_id_hash(rec.apple_id);
    let profile_id = format!("{SIGNING_METHOD_FREE}:{id_hash}");
    let expiration_ts = rec.installed_at + FREE_PROFILE_LIFETIME_SECS;
    let device_name = rec
        .device
        .name
        .clone()
        .unwrap_or_else(|| rec.device.udid.clone());
    let source_path = rec.stored_ipa.path.to_string_lossy().into_owned();

    let mut tx = pool.begin().await?;

    // 1. Device row (FK target for installations).
    sqlx::query(
        "INSERT INTO devices (udid, name, ios_version, pairing_status, last_seen) \
         VALUES (?1, ?2, ?3, 'paired', ?4) \
         ON CONFLICT(udid) DO UPDATE SET \
            name = excluded.name, \
            ios_version = COALESCE(excluded.ios_version, devices.ios_version), \
            last_seen = excluded.last_seen",
    )
    .bind(&rec.device.udid)
    .bind(&device_name)
    .bind(&rec.device.ios_version)
    .bind(rec.installed_at)
    .execute(&mut *tx)
    .await?;

    // 2. Signing profile (one per account for the free path; reused across apps).
    sqlx::query(
        "INSERT INTO signing_profiles (id, signing_method, apple_id_hash, team_id, secret_ref) \
         VALUES (?1, ?2, ?3, ?4, 'reside.apple_id') \
         ON CONFLICT(id) DO UPDATE SET \
            team_id = COALESCE(excluded.team_id, signing_profiles.team_id)",
    )
    .bind(&profile_id)
    .bind(SIGNING_METHOD_FREE)
    .bind(&id_hash)
    .bind(rec.team_id)
    .execute(&mut *tx)
    .await?;

    // 3. App row, keyed by bundle id. Re-importing refreshes the stored source.
    let existing_app: Option<i64> = sqlx::query_scalar("SELECT id FROM apps WHERE bundle_id = ?1")
        .bind(&rec.meta.bundle_id)
        .fetch_optional(&mut *tx)
        .await?;
    let app_id = match existing_app {
        Some(id) => {
            sqlx::query(
                "UPDATE apps SET display_name = ?1, version = ?2, \
                    source_ipa_sha256 = ?3, source_ipa_path = ?4 WHERE id = ?5",
            )
            .bind(&rec.meta.display_name)
            .bind(&rec.meta.version)
            .bind(&rec.stored_ipa.sha256)
            .bind(&source_path)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            id
        }
        None => {
            let res = sqlx::query(
                "INSERT INTO apps (display_name, bundle_id, version, source_ipa_sha256, source_ipa_path) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&rec.meta.display_name)
            .bind(&rec.meta.bundle_id)
            .bind(&rec.meta.version)
            .bind(&rec.stored_ipa.sha256)
            .bind(&source_path)
            .execute(&mut *tx)
            .await?;
            res.last_insert_rowid()
        }
    };

    // 4. Installation row, keyed by (app, device).
    let existing_inst: Option<i64> =
        sqlx::query_scalar("SELECT id FROM installations WHERE app_id = ?1 AND device_udid = ?2")
            .bind(app_id)
            .bind(&rec.device.udid)
            .fetch_optional(&mut *tx)
            .await?;
    let installation_id = match existing_inst {
        Some(id) => {
            sqlx::query(
                "UPDATE installations SET signing_method = ?1, install_ts = ?2, \
                    expiration_ts = ?3, cert_id = ?4, refresh_status = 'idle' WHERE id = ?5",
            )
            .bind(SIGNING_METHOD_FREE)
            .bind(rec.installed_at)
            .bind(expiration_ts)
            .bind(&profile_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            id
        }
        None => {
            let res = sqlx::query(
                "INSERT INTO installations \
                    (app_id, device_udid, signing_method, install_ts, expiration_ts, cert_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(app_id)
            .bind(&rec.device.udid)
            .bind(SIGNING_METHOD_FREE)
            .bind(rec.installed_at)
            .bind(expiration_ts)
            .bind(&profile_id)
            .execute(&mut *tx)
            .await?;
            res.last_insert_rowid()
        }
    };

    // 5. Activity log line so the install shows up in the recent-activity view.
    sqlx::query(
        "INSERT INTO activity_log (ts, severity, operation, message) \
         VALUES (?1, 'info', 'install', ?2)",
    )
    .bind(rec.installed_at)
    .bind(format!(
        "Installed {} ({})",
        rec.meta.display_name, rec.meta.bundle_id
    ))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(RecordedInstall {
        installation_id,
        app_id,
        expiration_ts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::path::PathBuf;

    fn record(version: &str) -> (DeviceRow, IpaMetadata, StoredIpa) {
        (
            DeviceRow {
                udid: "00008110-DEADBEEF".into(),
                name: Some("Eric's iPhone".into()),
                ios_version: Some("26.5".into()),
            },
            IpaMetadata {
                display_name: "Apollo".into(),
                bundle_id: "com.christianselig.apollo".into(),
                version: Some(version.into()),
            },
            StoredIpa {
                sha256: "abc123".into(),
                path: PathBuf::from("/tmp/abc123.ipa"),
                size: 42,
            },
        )
    }

    #[test]
    fn apple_id_hash_is_stable_and_normalized() {
        assert_eq!(
            apple_id_hash("User@Example.com "),
            apple_id_hash("user@example.com")
        );
        assert_ne!(apple_id_hash("a@b.com"), apple_id_hash("c@d.com"));
    }

    #[tokio::test]
    async fn records_and_then_updates_on_reinstall() {
        let pool = db::open_in_memory().await.unwrap();
        let (device, meta, ipa) = record("1.0");

        let first = record_install(
            &pool,
            &InstallRecord {
                device: &device,
                meta: &meta,
                stored_ipa: &ipa,
                apple_id: "eric@example.com",
                team_id: Some("NH3TYQN53H"),
                installed_at: 1_000,
            },
        )
        .await
        .unwrap();

        assert_eq!(first.expiration_ts, 1_000 + FREE_PROFILE_LIFETIME_SECS);

        // One row in each table.
        let apps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM apps")
            .fetch_one(&pool)
            .await
            .unwrap();
        let installs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM installations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!((apps, installs), (1, 1));

        // Re-install the same app (new version, later time) updates in place.
        let (_, meta2, _) = record("1.1");
        let second = record_install(
            &pool,
            &InstallRecord {
                device: &device,
                meta: &meta2,
                stored_ipa: &ipa,
                apple_id: "eric@example.com",
                team_id: None,
                installed_at: 50_000,
            },
        )
        .await
        .unwrap();

        assert_eq!(second.installation_id, first.installation_id);
        assert_eq!(second.app_id, first.app_id);
        assert_eq!(second.expiration_ts, 50_000 + FREE_PROFILE_LIFETIME_SECS);

        let apps_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM apps")
            .fetch_one(&pool)
            .await
            .unwrap();
        let installs_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM installations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!((apps_after, installs_after), (1, 1));

        // Version + expiry were refreshed.
        let (version, expiry): (Option<String>, i64) = sqlx::query_as(
            "SELECT a.version, i.expiration_ts FROM installations i \
             JOIN apps a ON a.id = i.app_id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(version.as_deref(), Some("1.1"));
        assert_eq!(expiry, 50_000 + FREE_PROFILE_LIFETIME_SECS);

        // Team id, once learned, is preserved when a later run doesn't supply it.
        let team: Option<String> = sqlx::query_scalar("SELECT team_id FROM signing_profiles")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(team.as_deref(), Some("NH3TYQN53H"));
    }
}
