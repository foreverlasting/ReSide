//! `reside-agent` — the headless background app-refresh agent (task 11c slice 2).
//!
//! This is the program the systemd timer (or XDG autostart entry) launches. It
//! builds the *same* runtime context the desktop app does — XDG paths, the
//! detected secret store, the SQLite pool — then calls the shared refresh engine
//! [`reside_core::refresh::refresh_due`]. It links none of the Tauri/GUI stack.
//!
//! Two modes (see [`reside_core::refresh::AgentMode`]):
//! - `run` (default): sweep once and exit. Used by the systemd *timer*, which
//!   owns the cadence.
//! - `loop`: sweep, sleep ~6h, repeat. Used by the autostart fallback, since
//!   autostart fires only once at login and there's no timer to re-trigger it.
//!
//! The agent never supplies a 2FA code (the engine enforces this), so a sweep
//! can fail loudly on a needs-a-human error but can never hang waiting for input.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reside_core::paths::Paths;
use reside_core::refresh::{self, AgentMode, RefreshSummary, DEFAULT_INTERVAL_HOURS};
use reside_core::secure_storage::SecureStore;
use reside_core::Result;

#[tokio::main]
async fn main() {
    // Honour RUST_LOG; default to info. Diagnostics go through tracing, never
    // println! — these land in the systemd journal under the user unit.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mode = AgentMode::from_arg(std::env::args().nth(1).as_deref());
    if let Err(err) = run(mode).await {
        // A hard failure to even start the sweep (e.g. DB unopenable) is worth a
        // non-zero exit so systemd records the unit as failed.
        tracing::error!(error = %err, "reside-agent exited with an error");
        std::process::exit(1);
    }
}

async fn run(mode: AgentMode) -> Result<()> {
    let ctx = AgentContext::resolve().await?;
    match mode {
        AgentMode::Run => {
            ctx.sweep_once().await?;
        }
        AgentMode::Loop => {
            let interval = Duration::from_secs(u64::from(DEFAULT_INTERVAL_HOURS) * 3600);
            tracing::info!(
                interval_hours = DEFAULT_INTERVAL_HOURS,
                "agent loop started"
            );
            loop {
                // A single failed sweep must not kill the loop — the engine
                // already backs off per-app; we just log and wait for the next tick.
                if let Err(err) = ctx.sweep_once().await {
                    tracing::error!(error = %err, "sweep failed; will retry next tick");
                }
                tokio::time::sleep(interval).await;
            }
        }
    }
    Ok(())
}

/// The shared bits a sweep needs, built once. Mirrors the app's `setup()` so the
/// agent and UI read/write the exact same database, secrets, and lock file.
struct AgentContext {
    paths: Paths,
    db: sqlx::SqlitePool,
    store: SecureStore,
}

impl AgentContext {
    async fn resolve() -> Result<Self> {
        let paths = Paths::resolve()?;
        paths.ensure_dirs()?;
        let (store, _keyring_warning) = SecureStore::detect(paths.data_dir().join("secrets"));
        let db = reside_core::db::open(paths.database_file()).await?;
        Ok(Self { paths, db, store })
    }

    /// One pass of the refresh engine under the shared single-writer lock.
    async fn sweep_once(&self) -> Result<RefreshSummary> {
        let summary = refresh::refresh_due(
            &self.db,
            &self.store,
            &self.paths.agent_pid_file(),
            unix_now(),
            refresh::REFRESH_LEAD_SECS,
        )
        .await;

        // Stop any Wi-Fi bridge this sweep started, whatever the outcome — the
        // agent is on-demand, so netmuxd must not linger between 6-hourly runs.
        reside_core::transport::muxer::shutdown().await;
        let summary = summary?;

        if !summary.ran {
            tracing::info!("sweep skipped: the app or another sweep holds the lock");
            return Ok(summary);
        }
        tracing::info!(
            attempted = summary.attempted,
            refreshed = summary.refreshed,
            "sweep complete"
        );

        // Best-effort desktop nudge for anything only the user can fix (e.g. a
        // re-auth). The engine has already written these to the activity log.
        for report in summary.needs_attention() {
            notify(
                "ReSide couldn't auto-refresh an app",
                &format!(
                    "{} needs you to open ReSide and sign in to Apple again to keep it working.",
                    report.display_name
                ),
            );
        }
        Ok(summary)
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Fire a desktop notification via `notify-send` if it's available. The agent is
/// headless (no Tauri notification plugin), and a host may lack `notify-send`
/// entirely, so this is strictly best-effort: failures are logged, never fatal.
fn notify(title: &str, body: &str) {
    match std::process::Command::new("notify-send")
        .args(["--app-name=ReSide", title, body])
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!(?status, "notify-send returned non-zero"),
        Err(e) => tracing::warn!(error = %e, "notify-send unavailable; relying on activity log"),
    }
}
