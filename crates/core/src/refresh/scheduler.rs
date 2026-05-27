//! Background job loop: profile-refresh vs cert-refresh paths, idempotent with
//! retry/backoff. A failed refresh must never delete the currently installed
//! app unless re-install succeeds. Phase 4 / task 11c.
//!
//! ## How a refresh works
//! A "refresh" is just task 11b run again, automatically: re-sign the *same*
//! stored IPA with the *same* stored credentials and re-install it, which resets
//! the free profile's 7-day clock. We reuse [`crate::signer::install`] and
//! [`crate::installs::record_install`] verbatim — there is no separate signing
//! path to keep in sync.
//!
//! ## Trigger-agnostic by design
//! Nothing here owns a timer. "Due" is derived purely from the
//! `installations.expiration_ts` already in SQLite, so the same engine serves
//! every trigger we might add later: a manual "Refresh now" button, a check on
//! app launch, or the systemd timer that makes refresh truly unattended (the
//! autopilot lands in [`super::agent`] next).
//!
//! ## 2FA must fail loudly, never hang
//! Unattended runs pass *no* 2FA code ([`crate::signer::InstallRequest::two_fa_code`]
//! is always `None`). On a trusted machine Apple doesn't re-challenge, so this is
//! a no-op; but if a challenge ever appears the signer returns
//! [`AppError::AppleAuth2faRequired`] and exits — we classify that as a
//! *needs-a-human* failure, mark the job `blocked` (no auto-retry), and surface
//! it. It can never silently wedge a background run.

use crate::error::{AppError, Result};
use crate::installs::{record_install, DeviceRow, InstallRecord};
use crate::ipa_meta::{read_ipa_metadata, IpaMetadata};
use crate::ipa_store::StoredIpa;
use crate::operation::{Operation, OperationStage};
use crate::proc_lock::ProcLock;
use crate::secure_storage::SecureStore;
use crate::signer::{self, InstallRequest};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

/// Refresh a free profile once it has this little time left. Free profiles last
/// 7 days; refreshing inside the final ~2 days leaves comfortable margin for a
/// 6-hourly check while avoiding needless re-signs early in the week.
pub const REFRESH_LEAD_SECS: i64 = 2 * 24 * 60 * 60;

/// The `jobs.kind` for the cheap, no-2FA weekly profile refresh (distinct from
/// the ~yearly `refresh_cert` path, which needs full re-auth — not in 11c).
const JOB_KIND_PROFILE: &str = "refresh_profile";

/// Backoff after a *transient* failure: `15min * 2^retries`, capped at 6h. A
/// flaky sign (pain #3) or a briefly-unplugged phone retries soon; a persistent
/// problem backs off so we don't hammer Apple or the device.
const BACKOFF_BASE_SECS: i64 = 15 * 60;
const BACKOFF_CAP_SECS: i64 = 6 * 60 * 60;

fn backoff_secs(retry_count: i64) -> i64 {
    let shift = retry_count.clamp(0, 16) as u32;
    BACKOFF_BASE_SECS
        .saturating_mul(1i64 << shift)
        .min(BACKOFF_CAP_SECS)
}

/// Why a failed refresh failed, which decides whether we retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Disposition {
    /// Recoverable on its own (device unplugged, flaky signing, rate-limit).
    /// Schedule a backoff retry; leave the working app untouched.
    Transient,
    /// Needs the user (2FA, wrong password, Apple weekly quota). Stop retrying
    /// and surface loudly — a background loop can't resolve these.
    NeedsAttention,
}

/// Classify an [`AppError`] from a refresh attempt. Defaults to `Transient` for
/// anything unrecognized so we retry rather than silently give up — except the
/// handful of errors only a human can clear.
fn disposition(err: &AppError) -> Disposition {
    use AppError::*;
    match err {
        AppleAuth2faRequired
        | AppleAuthCredentialsInvalid
        | AppleAuthProtocolChanged
        | AppleAppIdLimitReached
        | AppleDevDeviceRegLimitReached
        | SigningCertExpired => Disposition::NeedsAttention,
        _ => Disposition::Transient,
    }
}

/// One installation that is due (or overdue) for a profile refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueInstall {
    pub installation_id: i64,
    pub app_id: i64,
    pub bundle_id: String,
    pub display_name: String,
    pub device_udid: String,
    pub source_ipa_path: String,
    pub source_ipa_sha256: String,
    pub expiration_ts: i64,
}

/// Per-installation outcome, used to build the summary and (in the Tauri layer)
/// to decide what to notify the user about.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum RefreshOutcome {
    /// Re-signed and re-installed; the 7-day clock was reset.
    Refreshed { new_expiration_ts: i64 },
    /// Transient failure; will retry automatically after `next_run`.
    Retrying { category: String, next_run: i64 },
    /// Needs the user to act (e.g. sign in again); no auto-retry.
    NeedsAttention { category: String },
}

/// What one due install produced, paired with its identity for display/logging.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshReport {
    pub installation_id: i64,
    pub bundle_id: String,
    pub display_name: String,
    pub outcome: RefreshOutcome,
}

/// Result of a whole `refresh_due` batch.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSummary {
    /// `false` when another process (the agent, or another UI action) already
    /// held the single-writer lock and we declined to run concurrently.
    pub ran: bool,
    pub attempted: usize,
    pub refreshed: usize,
    pub reports: Vec<RefreshReport>,
}

impl RefreshSummary {
    /// Reports needing the user's attention — what the UI raises a notification for.
    pub fn needs_attention(&self) -> impl Iterator<Item = &RefreshReport> {
        self.reports
            .iter()
            .filter(|r| matches!(r.outcome, RefreshOutcome::NeedsAttention { .. }))
    }
}

/// Select installs whose free profile is within [`REFRESH_LEAD_SECS`] of expiry
/// (or already past), skipping ones currently refreshing, in a backoff window,
/// or `blocked` awaiting the user. Soonest-to-expire first.
pub async fn due_installs(pool: &SqlitePool, now: i64, lead: i64) -> Result<Vec<DueInstall>> {
    let rows = sqlx::query_as::<_, DueRow>(
        "SELECT i.id, i.app_id, a.bundle_id, a.display_name, i.device_udid, \
                a.source_ipa_path, a.source_ipa_sha256, i.expiration_ts \
         FROM installations i \
         JOIN apps a ON a.id = i.app_id \
         LEFT JOIN jobs j ON j.installation_id = i.id AND j.kind = ?1 \
         WHERE i.signing_method = 'free' \
           AND i.refresh_status != 'refreshing' \
           AND i.expiration_ts <= ?2 + ?3 \
           AND (j.status IS NULL OR j.status != 'blocked') \
           AND (j.next_run IS NULL OR j.next_run <= ?2) \
         ORDER BY i.expiration_ts ASC",
    )
    .bind(JOB_KIND_PROFILE)
    .bind(now)
    .bind(lead)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[derive(sqlx::FromRow)]
struct DueRow {
    id: i64,
    app_id: i64,
    bundle_id: String,
    display_name: String,
    device_udid: String,
    source_ipa_path: String,
    source_ipa_sha256: String,
    expiration_ts: i64,
}

impl From<DueRow> for DueInstall {
    fn from(r: DueRow) -> Self {
        Self {
            installation_id: r.id,
            app_id: r.app_id,
            bundle_id: r.bundle_id,
            display_name: r.display_name,
            device_udid: r.device_udid,
            source_ipa_path: r.source_ipa_path,
            source_ipa_sha256: r.source_ipa_sha256,
            expiration_ts: r.expiration_ts,
        }
    }
}

/// Re-sign and re-install a single installation by id, regardless of how close
/// it is to expiry (the manual "Refresh now" and the per-due-item path both call
/// this). On success the 7-day clock resets via [`record_install`]; on failure
/// the prior install is left intact and the `jobs` row records the disposition
/// (backoff for transient, `blocked` for needs-attention).
///
/// `ops` drives the `operation_{id}` progress stream when present (UI-triggered);
/// pass `None` for headless agent runs.
pub async fn refresh_installation(
    pool: &SqlitePool,
    store: &SecureStore,
    ops: Option<&Operation>,
    installation_id: i64,
    now: i64,
    creds_override: Option<&signer::AppleCredentials>,
) -> Result<i64> {
    let stage = |s, p, m: &str| {
        if let Some(op) = ops {
            op.stage(s, p, Some(m.to_string()));
        }
    };

    let install = load_installation(pool, installation_id).await?;
    set_refresh_status(pool, installation_id, "refreshing").await?;
    stage(OperationStage::Preparing, 0.1, "Preparing to re-sign…");

    // All the work that can fail; on any error we record the disposition and
    // restore a non-"refreshing" status before returning.
    let result = refresh_inner(pool, store, ops, &install, now, creds_override).await;

    match result {
        Ok(new_expiry) => {
            // record_install already set refresh_status back to 'idle'.
            clear_job(pool, installation_id, now).await?;
            log_activity(
                pool,
                now,
                "info",
                "refresh",
                None,
                &format!("Refreshed {} ({})", install.display_name, install.bundle_id),
            )
            .await?;
            stage(OperationStage::Done, 1.0, "Refreshed");
            if let Some(op) = ops {
                op.done();
            }
            Ok(new_expiry)
        }
        Err(err) => {
            let disp = disposition(&err);
            let category = err.category().as_key();
            match disp {
                Disposition::Transient => {
                    let next_run = record_retry(pool, installation_id, now).await?;
                    set_refresh_status(pool, installation_id, "idle").await?;
                    log_activity(
                        pool,
                        now,
                        "warn",
                        "refresh",
                        Some(category),
                        &format!(
                            "Refresh of {} failed ({category}); retrying after {}s",
                            install.bundle_id,
                            next_run - now
                        ),
                    )
                    .await?;
                    tracing::warn!(
                        installation_id,
                        bundle_id = %install.bundle_id,
                        category,
                        next_run,
                        "refresh failed; scheduled retry"
                    );
                }
                Disposition::NeedsAttention => {
                    block_job(pool, installation_id, now).await?;
                    set_refresh_status(pool, installation_id, "failed").await?;
                    log_activity(
                        pool,
                        now,
                        "error",
                        "refresh",
                        Some(category),
                        &format!(
                            "Refresh of {} needs attention ({category})",
                            install.bundle_id
                        ),
                    )
                    .await?;
                    tracing::error!(
                        installation_id,
                        bundle_id = %install.bundle_id,
                        category,
                        "refresh needs user attention; will not auto-retry"
                    );
                }
            }
            if let Some(op) = ops {
                op.fail(&err);
            }
            Err(err)
        }
    }
}

/// The fallible core of one refresh: load creds, read the stored IPA, sign +
/// install, and record the new install. Kept separate so the caller owns the
/// status/job bookkeeping for both success and every error path.
async fn refresh_inner(
    pool: &SqlitePool,
    store: &SecureStore,
    ops: Option<&Operation>,
    install: &DueInstall,
    now: i64,
    creds_override: Option<&signer::AppleCredentials>,
) -> Result<i64> {
    // Prefer caller-supplied in-memory (session) credentials; otherwise fall back
    // to the stored keyring account. The unattended agent always passes None, so
    // it can only run when creds are persisted — exactly the keyring tier.
    let creds = match creds_override {
        Some(c) => c.clone(),
        // No stored account → only the user can fix it. Surfaced as needs-attention.
        None => signer::load_credentials(store)?.ok_or(AppError::AppleAuthCredentialsInvalid)?,
    };

    // Pick the transport before signing: USB if the cable is in, else bring up
    // the Wi-Fi bridge (this may wait for mDNS discovery, hence the early stage).
    if let Some(op) = ops {
        op.stage(
            OperationStage::Preparing,
            0.15,
            Some("Locating your iPhone…".into()),
        );
    }
    let muxer_socket = crate::transport::muxer::route_to(&install.device_udid).await?;
    let via = if muxer_socket.is_some() {
        " over Wi-Fi"
    } else {
        ""
    };

    let ipa_path = PathBuf::from(&install.source_ipa_path);
    let size = std::fs::metadata(&ipa_path)
        .map_err(|e| {
            AppError::Internal(format!(
                "stored IPA for {} is missing ({}): {e}",
                install.bundle_id,
                ipa_path.display()
            ))
        })?
        .len();
    let meta: IpaMetadata = read_ipa_metadata(&ipa_path)?;
    let stored = StoredIpa {
        sha256: install.source_ipa_sha256.clone(),
        path: ipa_path.clone(),
        size,
    };

    if let Some(op) = ops {
        // First-ever signer run downloads a one-time ~150 MB Apple component
        // inside the blocking `install` call below (see signer::adi_libs_present);
        // fold the heads-up into the stage message that stays up for its whole
        // duration so a UI-triggered refresh doesn't look hung. Mirrors install_ipa.
        let msg = if signer::adi_libs_present() {
            format!("Re-signing {}{via}…", meta.display_name)
        } else {
            format!(
                "First sign-in: downloading a one-time component from Apple \
                 (~150 MB) and re-signing {}{via}… This happens only once.",
                meta.display_name
            )
        };
        op.stage(OperationStage::Signing, 0.3, Some(msg));
    }
    // Never supply a 2FA code unattended: a challenge must fail loudly, not hang.
    signer::install(&InstallRequest {
        creds: &creds,
        ipa_path: &ipa_path,
        udid: &install.device_udid,
        two_fa_code: None,
        muxer_socket: muxer_socket.as_deref(),
    })
    .await?;

    if let Some(op) = ops {
        op.stage(
            OperationStage::Installing,
            0.85,
            Some("Recording refresh…".into()),
        );
    }
    let device = device_row(pool, &install.device_udid).await?;
    let recorded = record_install(
        pool,
        &InstallRecord {
            device: &device,
            meta: &meta,
            stored_ipa: &stored,
            apple_id: &creds.apple_id,
            team_id: None,
            installed_at: now,
        },
    )
    .await?;
    Ok(recorded.expiration_ts)
}

/// Refresh every due install under the single-writer lock. Declines to run (with
/// `ran = false`) if another process already holds the lock, so the UI and the
/// background agent never refresh the same app at once.
pub async fn refresh_due(
    pool: &SqlitePool,
    store: &SecureStore,
    lock_path: &Path,
    now: i64,
    lead: i64,
    creds_override: Option<&signer::AppleCredentials>,
) -> Result<RefreshSummary> {
    let Some(lock) = ProcLock::try_acquire(lock_path)? else {
        tracing::info!("refresh skipped: another process holds the agent lock");
        return Ok(RefreshSummary::default());
    };

    let due = due_installs(pool, now, lead).await?;
    let mut summary = RefreshSummary {
        ran: true,
        attempted: due.len(),
        ..Default::default()
    };

    for d in due {
        let outcome =
            match refresh_installation(pool, store, None, d.installation_id, now, creds_override)
                .await
            {
                Ok(new_expiration_ts) => {
                    summary.refreshed += 1;
                    RefreshOutcome::Refreshed { new_expiration_ts }
                }
                Err(err) => match disposition(&err) {
                    Disposition::Transient => RefreshOutcome::Retrying {
                        category: err.category().as_key().to_string(),
                        next_run: now + backoff_secs(retry_count(pool, d.installation_id).await),
                    },
                    Disposition::NeedsAttention => RefreshOutcome::NeedsAttention {
                        category: err.category().as_key().to_string(),
                    },
                },
            };
        summary.reports.push(RefreshReport {
            installation_id: d.installation_id,
            bundle_id: d.bundle_id,
            display_name: d.display_name,
            outcome,
        });
    }

    lock.release();
    Ok(summary)
}

// --- DB helpers --------------------------------------------------------------

async fn load_installation(pool: &SqlitePool, installation_id: i64) -> Result<DueInstall> {
    let row = sqlx::query_as::<_, DueRow>(
        "SELECT i.id, i.app_id, a.bundle_id, a.display_name, i.device_udid, \
                a.source_ipa_path, a.source_ipa_sha256, i.expiration_ts \
         FROM installations i JOIN apps a ON a.id = i.app_id \
         WHERE i.id = ?1",
    )
    .bind(installation_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Internal(format!("no installation with id {installation_id}")))?;
    Ok(row.into())
}

/// Best-effort device identity from the `devices` row written at install time;
/// falls back to just the UDID if the row is somehow absent.
async fn device_row(pool: &SqlitePool, udid: &str) -> Result<DeviceRow> {
    let found: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT name, ios_version FROM devices WHERE udid = ?1")
            .bind(udid)
            .fetch_optional(pool)
            .await?;
    Ok(match found {
        Some((name, ios_version)) => DeviceRow {
            udid: udid.to_string(),
            name: Some(name),
            ios_version,
        },
        None => DeviceRow {
            udid: udid.to_string(),
            name: None,
            ios_version: None,
        },
    })
}

async fn set_refresh_status(pool: &SqlitePool, installation_id: i64, status: &str) -> Result<()> {
    sqlx::query("UPDATE installations SET refresh_status = ?1 WHERE id = ?2")
        .bind(status)
        .bind(installation_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Upsert the `refresh_profile` job row for an installation, adjusting
/// `retry_count` per the `bump_retry`/`reset_retry` flags. There is no unique
/// constraint on (installation_id, kind), so we select-then-insert/update like
/// [`crate::installs`].
async fn upsert_job(
    pool: &SqlitePool,
    installation_id: i64,
    now: i64,
    status: &str,
    next_run: Option<i64>,
    bump_retry: bool,
    reset_retry: bool,
) -> Result<i64> {
    let existing: Option<(i64, i64)> =
        sqlx::query_as("SELECT id, retry_count FROM jobs WHERE installation_id = ?1 AND kind = ?2")
            .bind(installation_id)
            .bind(JOB_KIND_PROFILE)
            .fetch_optional(pool)
            .await?;

    let retry_count = match existing {
        Some(_) if reset_retry => 0,
        Some((_, rc)) if bump_retry => rc + 1,
        Some((_, rc)) => rc,
        None if bump_retry => 1,
        None => 0,
    };

    match existing {
        Some((id, _)) => {
            sqlx::query(
                "UPDATE jobs SET status = ?1, next_run = ?2, last_run = ?3, retry_count = ?4 \
                 WHERE id = ?5",
            )
            .bind(status)
            .bind(next_run)
            .bind(now)
            .bind(retry_count)
            .bind(id)
            .execute(pool)
            .await?;
        }
        None => {
            sqlx::query(
                "INSERT INTO jobs (installation_id, kind, next_run, last_run, retry_count, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(installation_id)
            .bind(JOB_KIND_PROFILE)
            .bind(next_run)
            .bind(now)
            .bind(retry_count)
            .bind(status)
            .execute(pool)
            .await?;
        }
    }
    Ok(retry_count)
}

/// Mark the job done after a successful refresh: clear backoff, reset retries.
async fn clear_job(pool: &SqlitePool, installation_id: i64, now: i64) -> Result<()> {
    upsert_job(pool, installation_id, now, "done", None, false, true).await?;
    Ok(())
}

/// Record a transient failure and schedule the next attempt; returns `next_run`.
async fn record_retry(pool: &SqlitePool, installation_id: i64, now: i64) -> Result<i64> {
    // Bump first so the backoff reflects this attempt.
    let rc = upsert_job(
        pool,
        installation_id,
        now,
        "pending",
        Some(now),
        true,
        false,
    )
    .await?;
    let next_run = now + backoff_secs(rc);
    upsert_job(
        pool,
        installation_id,
        now,
        "pending",
        Some(next_run),
        false,
        false,
    )
    .await?;
    Ok(next_run)
}

/// Mark the job blocked: needs the user, do not auto-retry.
async fn block_job(pool: &SqlitePool, installation_id: i64, now: i64) -> Result<()> {
    upsert_job(pool, installation_id, now, "blocked", None, false, false).await?;
    Ok(())
}

async fn retry_count(pool: &SqlitePool, installation_id: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT retry_count FROM jobs WHERE installation_id = ?1 AND kind = ?2",
    )
    .bind(installation_id)
    .bind(JOB_KIND_PROFILE)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(0)
}

async fn log_activity(
    pool: &SqlitePool,
    ts: i64,
    severity: &str,
    operation: &str,
    error_category: Option<&str>,
    message: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO activity_log (ts, severity, operation, error_category, message) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(ts)
    .bind(severity)
    .bind(operation)
    .bind(error_category)
    .bind(message)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::installs::{record_install, DeviceRow, InstallRecord, FREE_PROFILE_LIFETIME_SECS};
    use crate::ipa_meta::IpaMetadata;
    use crate::ipa_store::StoredIpa;
    use std::path::PathBuf;

    /// Insert one free install expiring 7 days after `installed_at`, returning
    /// its id. `key` distinguishes installs: `record_install` is keyed by
    /// (app, device), so distinct keys produce distinct installation rows.
    /// Goes through `record_install` so the schema/relations match production.
    async fn seed_install(pool: &SqlitePool, key: &str, installed_at: i64) -> i64 {
        let device = DeviceRow {
            udid: format!("00008150-{key}"),
            name: Some("Test iPhone".into()),
            ios_version: Some("26.5".into()),
        };
        let meta = IpaMetadata {
            display_name: format!("App {key}"),
            bundle_id: format!("com.example.{key}"),
            version: Some("1.0".into()),
        };
        let ipa = StoredIpa {
            sha256: "deadbeef".into(),
            path: PathBuf::from("/tmp/deadbeef.ipa"),
            size: 1,
        };
        record_install(
            pool,
            &InstallRecord {
                device: &device,
                meta: &meta,
                stored_ipa: &ipa,
                apple_id: "eric@example.com",
                team_id: None,
                installed_at,
            },
        )
        .await
        .unwrap()
        .installation_id
    }

    #[test]
    fn backoff_grows_then_caps() {
        assert_eq!(backoff_secs(0), BACKOFF_BASE_SECS);
        assert_eq!(backoff_secs(1), BACKOFF_BASE_SECS * 2);
        assert_eq!(backoff_secs(2), BACKOFF_BASE_SECS * 4);
        // Eventually pinned to the cap, with no overflow at large counts.
        assert_eq!(backoff_secs(100), BACKOFF_CAP_SECS);
    }

    #[test]
    fn disposition_splits_human_errors_from_retryable() {
        assert_eq!(
            disposition(&AppError::AppleAuth2faRequired),
            Disposition::NeedsAttention
        );
        assert_eq!(
            disposition(&AppError::AppleAuthCredentialsInvalid),
            Disposition::NeedsAttention
        );
        assert_eq!(
            disposition(&AppError::AppleAppIdLimitReached),
            Disposition::NeedsAttention
        );
        // Device / infra problems are worth retrying.
        assert_eq!(
            disposition(&AppError::DeviceOffline),
            Disposition::Transient
        );
        assert_eq!(
            disposition(&AppError::Internal("boom".into())),
            Disposition::Transient
        );
    }

    #[tokio::test]
    async fn due_picker_respects_lead_window_and_states() {
        let pool = db::open_in_memory().await.unwrap();
        let now = 1_000_000;

        // Fresh install (expires in 7 days): not due under a 2-day lead.
        let fresh = seed_install(&pool, "fresh", now).await;
        // An install expiring in 1 day: due.
        let soon = seed_install(&pool, "soon", now).await;
        sqlx::query("UPDATE installations SET expiration_ts = ?1 WHERE id = ?2")
            .bind(now + 24 * 3600)
            .bind(soon)
            .execute(&pool)
            .await
            .unwrap();

        let due = due_installs(&pool, now, REFRESH_LEAD_SECS).await.unwrap();
        let ids: Vec<i64> = due.iter().map(|d| d.installation_id).collect();
        assert!(ids.contains(&soon), "soon-expiring install should be due");
        assert!(!ids.contains(&fresh), "fresh install should not be due");

        // Mark the due one as refreshing → excluded.
        set_refresh_status(&pool, soon, "refreshing").await.unwrap();
        let due = due_installs(&pool, now, REFRESH_LEAD_SECS).await.unwrap();
        assert!(due.iter().all(|d| d.installation_id != soon));

        // Blocked job → excluded even though it's expiring soon.
        set_refresh_status(&pool, soon, "failed").await.unwrap();
        block_job(&pool, soon, now).await.unwrap();
        let due = due_installs(&pool, now, REFRESH_LEAD_SECS).await.unwrap();
        assert!(due.iter().all(|d| d.installation_id != soon));
    }

    #[tokio::test]
    async fn due_picker_excludes_backoff_then_includes_after_next_run() {
        let pool = db::open_in_memory().await.unwrap();
        let now = 2_000_000;
        let id = seed_install(&pool, "backoff", now).await;
        sqlx::query("UPDATE installations SET expiration_ts = ?1 WHERE id = ?2")
            .bind(now) // already at expiry → in the lead window
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        // Schedule a retry in the future: excluded now…
        let next_run = record_retry(&pool, id, now).await.unwrap();
        assert!(next_run > now);
        let due = due_installs(&pool, now, REFRESH_LEAD_SECS).await.unwrap();
        assert!(due.iter().all(|d| d.installation_id != id));

        // …included once we're past next_run.
        let due = due_installs(&pool, next_run, REFRESH_LEAD_SECS)
            .await
            .unwrap();
        assert!(due.iter().any(|d| d.installation_id == id));
    }

    #[tokio::test]
    async fn refresh_due_declines_when_lock_held() {
        let pool = db::open_in_memory().await.unwrap();
        let store = SecureStore::File(tempfile::tempdir().unwrap().path().join("secrets"));
        let tmp = tempfile::tempdir().unwrap();
        let lock_path = tmp.path().join("agent.pid");

        // Hold the lock as if another process were mid-refresh.
        let held = ProcLock::try_acquire(&lock_path).unwrap().unwrap();
        let summary = refresh_due(&pool, &store, &lock_path, 1_000, REFRESH_LEAD_SECS, None)
            .await
            .unwrap();
        assert!(!summary.ran, "must decline while the lock is held");
        held.release();
    }

    #[tokio::test]
    async fn missing_credentials_block_the_job() {
        let pool = db::open_in_memory().await.unwrap();
        // Empty secret store → no Apple account stored.
        let store = SecureStore::File(tempfile::tempdir().unwrap().path().join("secrets"));
        let now = 3_000_000;
        let id = seed_install(&pool, "nocreds", now).await;

        let err = refresh_installation(&pool, &store, None, id, now, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::AppleAuthCredentialsInvalid));

        // The job is blocked (needs the user) and the app left non-"refreshing".
        let status: String =
            sqlx::query_scalar("SELECT status FROM jobs WHERE installation_id = ?1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "blocked");
        let rs: String =
            sqlx::query_scalar("SELECT refresh_status FROM installations WHERE id = ?1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(rs, "failed");

        // The original install row (and its expiry) is untouched — nothing deleted.
        let expiry: i64 =
            sqlx::query_scalar("SELECT expiration_ts FROM installations WHERE id = ?1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(expiry, now + FREE_PROFILE_LIFETIME_SECS);
    }
}
