//! Apple Developer Services quota tracking via `apple_quota_events`.
//!
//! ⚠️ **PARKED** — quota tracking belongs to the abandoned native-signing
//! path (the live app delegates to the forked Sideloader and surfaces Apple's
//! own throttle errors directly). Kept for reference. See [`super`] and
//! [`crate::signer`].
//!
//! Free Apple IDs are throttled by Apple: at most 10 device registrations and
//! 10 App ID creations per Apple ID per rolling 7-day window. We mirror those
//! counts locally so the free-Apple-ID flow can fail fast with an actionable
//! error *before* spending a network round-trip on a call Apple will reject
//! (plan.md §Apple Developer Services quotas, §Known Foot-Guns: "always check
//! before calling, always log after a successful call").
//!
//! The 3-active-apps-per-device limit is enforced by Apple directly and surfaced
//! elsewhere as a bundle-id picker — it is not a rolling-window quota, so it is
//! not tracked here.

use crate::error::{AppError, Result};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

/// Length of the rolling quota window, in seconds (7 days).
pub const WINDOW_SECS: i64 = 7 * 24 * 60 * 60;
/// Apple's per-Apple-ID limit on device registrations per rolling window.
pub const MAX_DEVICE_REGISTRATIONS: i64 = 10;
/// Apple's per-Apple-ID limit on App ID creations per rolling window.
pub const MAX_APP_IDS: i64 = 10;

/// A quota-consuming Apple Developer Services action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaEvent {
    /// Registering a device UDID with Apple.
    DeviceRegistered,
    /// Creating a new App ID (explicit identifier) with Apple.
    AppIdCreated,
}

impl QuotaEvent {
    /// The `event_type` string persisted in `apple_quota_events` (must match the
    /// table's CHECK constraint).
    fn as_db_str(self) -> &'static str {
        match self {
            QuotaEvent::DeviceRegistered => "device_registered",
            QuotaEvent::AppIdCreated => "app_id_created",
        }
    }

    fn limit(self) -> i64 {
        match self {
            QuotaEvent::DeviceRegistered => MAX_DEVICE_REGISTRATIONS,
            QuotaEvent::AppIdCreated => MAX_APP_IDS,
        }
    }

    /// The fail-fast error to raise when this event's window is full.
    fn limit_error(self) -> AppError {
        match self {
            QuotaEvent::DeviceRegistered => AppError::AppleDevDeviceRegLimitReached,
            QuotaEvent::AppIdCreated => AppError::AppleAppIdLimitReached,
        }
    }
}

/// How much of each quota an Apple ID has consumed in the current window.
/// Surfaced to the UI as the free-tier limits banner (plan.md §Free Apple ID
/// limits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaStatus {
    pub devices_used: i64,
    pub devices_limit: i64,
    pub app_ids_used: i64,
    pub app_ids_limit: i64,
}

impl QuotaStatus {
    pub fn devices_remaining(&self) -> i64 {
        (self.devices_limit - self.devices_used).max(0)
    }
    pub fn app_ids_remaining(&self) -> i64 {
        (self.app_ids_limit - self.app_ids_used).max(0)
    }
}

/// Hash an Apple ID into the opaque `apple_id_hash` persisted in SQLite. The
/// raw Apple ID is a secret and must never be stored (plan.md §Secrets &
/// Redaction); only this hash is keyed on.
pub fn account_hash(apple_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"reside-apple-id:");
    hasher.update(apple_id.trim().to_lowercase().as_bytes());
    hex::encode(hasher.finalize())
}

/// Current wall-clock time as a Unix timestamp in seconds.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Count `event` occurrences for `apple_id_hash` within the rolling window
/// ending at `now`.
async fn count_in_window(
    pool: &SqlitePool,
    apple_id_hash: &str,
    event: QuotaEvent,
    now: i64,
) -> Result<i64> {
    let since = now - WINDOW_SECS;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM apple_quota_events \
         WHERE apple_id_hash = ?1 AND event_type = ?2 AND ts > ?3",
    )
    .bind(apple_id_hash)
    .bind(event.as_db_str())
    .bind(since)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Fail fast if registering one more `event` for this Apple ID would exceed
/// Apple's rolling-window quota. Call this *before* the Apple API request.
pub async fn check(pool: &SqlitePool, apple_id_hash: &str, event: QuotaEvent) -> Result<()> {
    check_at(pool, apple_id_hash, event, now_unix()).await
}

/// [`check`] with an explicit `now` (for tests and deterministic windows).
pub async fn check_at(
    pool: &SqlitePool,
    apple_id_hash: &str,
    event: QuotaEvent,
    now: i64,
) -> Result<()> {
    let used = count_in_window(pool, apple_id_hash, event, now).await?;
    if used >= event.limit() {
        return Err(event.limit_error());
    }
    Ok(())
}

/// Log a successful quota-consuming call. Call this *after* Apple confirms the
/// action so a failed call never burns local quota.
pub async fn record(pool: &SqlitePool, apple_id_hash: &str, event: QuotaEvent) -> Result<()> {
    record_at(pool, apple_id_hash, event, now_unix()).await
}

/// [`record`] with an explicit `now` (for tests).
pub async fn record_at(
    pool: &SqlitePool,
    apple_id_hash: &str,
    event: QuotaEvent,
    now: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO apple_quota_events (apple_id_hash, event_type, ts) VALUES (?1, ?2, ?3)",
    )
    .bind(apple_id_hash)
    .bind(event.as_db_str())
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Snapshot both quotas for an Apple ID in the window ending now.
pub async fn status(pool: &SqlitePool, apple_id_hash: &str) -> Result<QuotaStatus> {
    status_at(pool, apple_id_hash, now_unix()).await
}

/// [`status`] with an explicit `now` (for tests).
pub async fn status_at(pool: &SqlitePool, apple_id_hash: &str, now: i64) -> Result<QuotaStatus> {
    let devices_used =
        count_in_window(pool, apple_id_hash, QuotaEvent::DeviceRegistered, now).await?;
    let app_ids_used = count_in_window(pool, apple_id_hash, QuotaEvent::AppIdCreated, now).await?;
    Ok(QuotaStatus {
        devices_used,
        devices_limit: MAX_DEVICE_REGISTRATIONS,
        app_ids_used,
        app_ids_limit: MAX_APP_IDS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    const NOW: i64 = 1_700_000_000;

    fn hash() -> String {
        account_hash("test@example.com")
    }

    #[test]
    fn account_hash_is_stable_and_case_insensitive() {
        assert_eq!(account_hash("Foo@Bar.com"), account_hash("foo@bar.com  "));
        assert_ne!(account_hash("a@b.com"), account_hash("c@d.com"));
        // 32-byte SHA-256 → 64 hex chars; never the raw address.
        assert_eq!(hash().len(), 64);
        assert!(!hash().contains('@'));
    }

    #[tokio::test]
    async fn check_passes_until_limit_then_fails() {
        let pool = db::open_in_memory().await.unwrap();
        let h = hash();

        // Fill the device quota right up to the limit.
        for _ in 0..MAX_DEVICE_REGISTRATIONS {
            check_at(&pool, &h, QuotaEvent::DeviceRegistered, NOW)
                .await
                .expect("under limit");
            record_at(&pool, &h, QuotaEvent::DeviceRegistered, NOW)
                .await
                .unwrap();
        }

        // The 11th must fail fast with the device-limit error.
        let err = check_at(&pool, &h, QuotaEvent::DeviceRegistered, NOW)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::AppleDevDeviceRegLimitReached));

        // App-ID quota is independent and still open.
        check_at(&pool, &h, QuotaEvent::AppIdCreated, NOW)
            .await
            .expect("app-id quota untouched");
    }

    #[tokio::test]
    async fn events_outside_the_window_do_not_count() {
        let pool = db::open_in_memory().await.unwrap();
        let h = hash();

        // Max out App IDs, but all just over 7 days before NOW.
        let stale = NOW - WINDOW_SECS - 1;
        for _ in 0..MAX_APP_IDS {
            record_at(&pool, &h, QuotaEvent::AppIdCreated, stale)
                .await
                .unwrap();
        }

        // The window has rolled past them, so quota is fully available again.
        check_at(&pool, &h, QuotaEvent::AppIdCreated, NOW)
            .await
            .expect("stale events expired from window");
        let st = status_at(&pool, &h, NOW).await.unwrap();
        assert_eq!(st.app_ids_used, 0);
        assert_eq!(st.app_ids_remaining(), MAX_APP_IDS);
    }

    #[tokio::test]
    async fn quotas_are_scoped_per_apple_id() {
        let pool = db::open_in_memory().await.unwrap();
        let a = account_hash("a@example.com");
        let b = account_hash("b@example.com");

        for _ in 0..MAX_DEVICE_REGISTRATIONS {
            record_at(&pool, &a, QuotaEvent::DeviceRegistered, NOW)
                .await
                .unwrap();
        }

        // Account A is maxed; account B is unaffected.
        assert!(check_at(&pool, &a, QuotaEvent::DeviceRegistered, NOW)
            .await
            .is_err());
        check_at(&pool, &b, QuotaEvent::DeviceRegistered, NOW)
            .await
            .expect("other account has its own quota");
    }
}
