//! ReSide Tauri shell. Commands are thin shims over `reside-core`; this layer
//! owns IPC, the operation-event bridge, and redaction of serialized payloads.

pub mod redaction;

use reside_core::ipa_store::IpaStore;
use reside_core::operation::OperationChannel;
use reside_core::paths::Paths;
use reside_core::secure_storage::SecureStore;
use reside_core::transport::tunneld::TunnelManager;
use reside_core::AppError;
use sqlx::SqlitePool;
use tauri::{Emitter, Manager};

/// Shared application state. Some fields are wired here but first read in later
/// phases (device/signing/refresh), hence the allow.
#[allow(dead_code)]
pub struct AppState {
    paths: Paths,
    db: SqlitePool,
    store: SecureStore,
    /// In-memory Apple credentials for this app session only — the "remember for
    /// this session" / "ask each time" tiers. Never written to disk; cleared on
    /// sign-out and when the process exits. `None` means fall back to the keyring.
    session_creds: std::sync::Mutex<Option<reside_core::signer::AppleCredentials>>,
    ops: OperationChannel,
    tunnels: TunnelManager,
    ipa_store: IpaStore,
}

impl AppState {
    /// Resolve the credentials to use for an operation: the in-memory session
    /// credentials if present (most recent, intentional), else the stored keyring
    /// account. Clones out under the lock so nothing is held across an `.await`.
    fn resolve_credentials(&self) -> Option<reside_core::signer::AppleCredentials> {
        if let Some(c) = self
            .session_creds
            .lock()
            .expect("session creds mutex")
            .clone()
        {
            return Some(c);
        }
        reside_core::signer::load_credentials(&self.store)
            .ok()
            .flatten()
    }

    /// Just the in-memory session credentials (no keyring fallback), for passing
    /// as the refresh engine's override — `None` lets the engine load the keyring
    /// itself. Keeps "override == session only" semantics out of the engine.
    fn resolve_session_only(&self) -> Option<reside_core::signer::AppleCredentials> {
        self.session_creds
            .lock()
            .expect("session creds mutex")
            .clone()
    }
}

/// UI-safe error returned across the IPC boundary: category key + remediation,
/// never raw upstream error text or secrets.
#[derive(serde::Serialize)]
pub struct CommandError {
    category: String,
    remediation: String,
}

impl From<AppError> for CommandError {
    fn from(e: AppError) -> Self {
        let r = e.report();
        Self {
            category: r.category,
            remediation: r.remediation,
        }
    }
}

type CmdResult<T> = Result<T, CommandError>;

// ---------------------------------------------------------------------------
// Commands (Phase 0 surface; most are stubs filled in by later phases).
// The full command surface is enumerated in plan.md §Tauri command surface.
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct SetupReport {
    items: Vec<reside_core::setup::permissions::SetupCheck>,
    ok: usize,
    warn: usize,
}

/// Runs the implemented system checks (Phase 1 seed in `core::setup::permissions`).
/// `keyring_available` reflects the store we actually selected at startup.
#[tauri::command]
async fn run_setup_check(state: tauri::State<'_, AppState>) -> CmdResult<SetupReport> {
    use reside_core::setup::permissions::{run_checks, CheckStatus};
    let keyring_available = matches!(state.store, SecureStore::Keyring);
    let items = run_checks(keyring_available);
    let warn = items
        .iter()
        .filter(|c| c.status == CheckStatus::Warn)
        .count();
    let ok = items.len() - warn;
    Ok(SetupReport { items, ok, warn })
}

#[derive(serde::Serialize)]
pub struct TunnelPill {
    connected: bool,
}

/// Aggregate tunnel state for the (non device-scoped) titlebar pill: connected
/// if the manager currently holds any live tunnel.
#[tauri::command]
async fn get_tunnel_status(state: tauri::State<'_, AppState>) -> CmdResult<TunnelPill> {
    Ok(TunnelPill {
        connected: state.tunnels.any_connected().await,
    })
}

/// Establish an RSD tunnel to a paired device over USB (CoreDeviceProxy →
/// software tunnel → RSD handshake) and return its endpoint + discovered
/// services. Requires a stored pair record. Device-dependent.
#[tauri::command]
async fn establish_tunnel(
    state: tauri::State<'_, AppState>,
    udid: String,
) -> CmdResult<reside_core::transport::tunneld::TunnelStatus> {
    Ok(state.tunnels.connect_usb(&udid).await?)
}

/// Enumerate connected devices over usbmuxd (USB + network).
#[tauri::command]
async fn list_devices() -> CmdResult<Vec<reside_core::device::DeviceInfo>> {
    Ok(reside_core::device::list_devices().await?)
}

/// Pair with a device over USB. Blocks until the user responds to the on-device
/// "Trust This Computer?" dialog, then persists the pairing record.
#[tauri::command]
async fn pair_device(udid: String) -> CmdResult<()> {
    reside_core::device::pair_device(&udid).await?;
    Ok(())
}

/// Read whether Developer Mode is enabled on a (paired) device. Requires a
/// stored pair record; iOS 17.4+ needs Developer Mode for install flows.
#[tauri::command]
async fn developer_mode_status(udid: String) -> CmdResult<bool> {
    Ok(reside_core::device::developer_mode_status(&udid).await?)
}

/// Browse the local network (mDNS) for RemoteXPC-capable iOS endpoints. Reports
/// whether any device is reachable over Wi-Fi — the pre-tunnel signal for Wi-Fi
/// refresh. Not yet device-scoped; mapping an endpoint to a UDID needs the Wi-Fi
/// tunnel slice. Network-dependent.
#[tauri::command]
async fn check_wifi_availability(
) -> CmdResult<reside_core::transport::mdns_discovery::WifiAvailability> {
    Ok(reside_core::transport::mdns_discovery::check_wifi_availability().await?)
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct ActivityRow {
    ts: i64,
    severity: String,
    operation: Option<String>,
    error_category: Option<String>,
    message: Option<String>,
}

/// Reads the most recent activity-log rows. This one is real already — it just
/// returns an empty list until features start writing to the log.
#[tauri::command]
async fn get_activity_log(state: tauri::State<'_, AppState>) -> CmdResult<Vec<ActivityRow>> {
    let rows = sqlx::query_as::<_, ActivityRow>(
        "SELECT ts, severity, operation, error_category, message \
         FROM activity_log ORDER BY ts DESC LIMIT 200",
    )
    .fetch_all(&state.db)
    .await
    .map_err(AppError::from)?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Apple account + sign/install (task 11b). ReSide stores the user's Apple ID
// credentials locally and drives the forked Sideloader signer to sign + install
// an IPA over USB; the result is recorded in SQLite for the Dashboard and the
// (upcoming) refresh agent.
// ---------------------------------------------------------------------------

/// Open a native file picker for an `.ipa`. Returns the chosen path, or `None`
/// if the user cancelled. Run on the Rust side so the real filesystem path
/// stays available to the signer (a browser `<input type=file>` would not
/// expose it).
#[tauri::command]
async fn pick_ipa(app: tauri::AppHandle) -> CmdResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("iOS app", &["ipa"])
        .pick_file(move |picked| {
            let _ = tx.send(picked.map(|p| p.to_string()));
        });
    rx.await
        .map_err(|_| AppError::Internal("file dialog closed unexpectedly".into()).into())
}

/// Whether an Apple ID is stored (i.e. the user has "signed in" to ReSide).
#[tauri::command]
async fn is_signed_in(state: tauri::State<'_, AppState>) -> CmdResult<bool> {
    Ok(state.resolve_credentials().is_some())
}

/// How the user wants their Apple credentials remembered. `Keyring` persists to
/// the system keyring (survives restart, lets the background agent auto-refresh);
/// `Session` keeps them in memory for this app run only (nothing on disk). The
/// "ask every time" tier is `Session` plus the UI signing out after each op.
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum RememberMode {
    #[default]
    Keyring,
    Session,
}

/// Credential state for the UI: where (if anywhere) creds are held, and whether
/// the keyring is even available (so the UI can disable the "remember" option).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    /// "keyring" | "session" | "none".
    mode: &'static str,
    keyring_available: bool,
}

#[tauri::command]
async fn credential_status(state: tauri::State<'_, AppState>) -> CmdResult<CredentialStatus> {
    let keyring_available = matches!(state.store, SecureStore::Keyring);
    let mode = if reside_core::signer::load_credentials(&state.store)?.is_some() {
        "keyring"
    } else if state
        .session_creds
        .lock()
        .expect("session creds mutex")
        .is_some()
    {
        "session"
    } else {
        "none"
    };
    Ok(CredentialStatus {
        mode,
        keyring_available,
    })
}

/// Store the Apple ID credentials per the chosen [`RememberMode`]. Authentication
/// itself happens during install/refresh, so this just stashes them. `Keyring`
/// errors with `KeyringUnavailable` when no keyring is present — the UI offers
/// that option only when one is.
#[tauri::command]
async fn set_apple_credentials(
    state: tauri::State<'_, AppState>,
    apple_id: String,
    password: String,
    remember: Option<RememberMode>,
) -> CmdResult<()> {
    let creds = reside_core::signer::AppleCredentials {
        apple_id,
        password: reside_core::Secret::new(password),
    };
    match remember.unwrap_or_default() {
        RememberMode::Keyring => reside_core::signer::store_credentials(&state.store, &creds)?,
        RememberMode::Session => {
            *state.session_creds.lock().expect("session creds mutex") = Some(creds);
        }
    }
    Ok(())
}

/// Forget the Apple ID credentials — both the persisted keyring copy and any
/// in-memory session copy.
#[tauri::command]
async fn sign_out(state: tauri::State<'_, AppState>) -> CmdResult<()> {
    reside_core::signer::clear_credentials(&state.store)?;
    *state.session_creds.lock().expect("session creds mutex") = None;
    Ok(())
}

/// List the Apple account's development certificates (Settings → Certificates).
/// Returns `AppleAuthCredentialsInvalid` if no Apple ID is stored yet, or the
/// auth/2FA category if Apple challenges the login.
#[tauri::command]
async fn list_certificates(
    state: tauri::State<'_, AppState>,
) -> CmdResult<Vec<reside_core::signer::CertInfo>> {
    let Some(creds) = state.resolve_credentials() else {
        return Err(AppError::AppleAuthCredentialsInvalid.into());
    };
    Ok(reside_core::signer::list_certs(&creds).await?)
}

/// Revoke the certificate with `serial_number` (from [`list_certificates`]).
/// This is the way out of `AppleCertLimitReached`: free Apple IDs cap at ~2
/// certs, so revoking a stale one lets signing proceed again.
#[tauri::command]
async fn revoke_certificate(
    state: tauri::State<'_, AppState>,
    serial_number: String,
) -> CmdResult<()> {
    let Some(creds) = state.resolve_credentials() else {
        return Err(AppError::AppleAuthCredentialsInvalid.into());
    };
    reside_core::signer::revoke_cert(&creds, &serial_number).await?;
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    installation_id: i64,
    display_name: String,
    bundle_id: String,
    expiration_ts: i64,
}

/// Sign and install `path` onto `udid` via the forked signer, then record it.
///
/// Progress is emitted on `operation_{operation_id}`; the frontend subscribes
/// before invoking so it can show stages. If Apple wants a 2FA code this returns
/// the `AppleAuth2FARequired` category — re-invoke with `two_fa_code` set.
#[tauri::command]
async fn install_ipa(
    state: tauri::State<'_, AppState>,
    operation_id: String,
    path: String,
    udid: String,
    two_fa_code: Option<String>,
) -> CmdResult<InstallOutcome> {
    use reside_core::operation::OperationStage;
    use reside_core::signer::{self, InstallRequest};

    let op = state.ops.start(operation_id);

    let run = async {
        let Some(creds) = state.resolve_credentials() else {
            return Err(AppError::AppleAuthCredentialsInvalid);
        };

        op.stage(OperationStage::Preparing, 0.1, Some("Reading app…".into()));
        let stored = state.ipa_store.store_file(&path)?;
        let meta = reside_core::ipa_meta::read_ipa_metadata(&stored.path)?;

        // Pick the transport (USB cable, else bring up the Wi-Fi bridge) before
        // signing so the fork is pointed at the right muxer.
        op.stage(
            OperationStage::Preparing,
            0.2,
            Some("Locating your iPhone…".into()),
        );
        let muxer_socket = reside_core::transport::muxer::route_to(&udid).await?;
        let via = if muxer_socket.is_some() {
            " over Wi-Fi"
        } else {
            ""
        };

        // On a brand-new install the fork downloads a one-time ~150 MB Apple
        // component before it can sign (see signer::adi_libs_present). That
        // happens inside the blocking `install` call below, so fold the heads-up
        // into the stage message that stays on screen for its whole duration —
        // otherwise the first sign-in looks hung.
        let signing_msg = if signer::adi_libs_present() {
            format!("Signing {}{via}…", meta.display_name)
        } else {
            format!(
                "First sign-in: downloading a one-time component from Apple \
                 (~150 MB) and signing {}{via}… This happens only once.",
                meta.display_name
            )
        };
        op.stage(OperationStage::Signing, 0.3, Some(signing_msg));
        let req = InstallRequest {
            creds: &creds,
            ipa_path: &stored.path,
            udid: &udid,
            two_fa_code: two_fa_code.as_deref(),
            muxer_socket: muxer_socket.as_deref(),
        };
        signer::install(&req).await?;

        op.stage(
            OperationStage::Installing,
            0.85,
            Some("Recording install…".into()),
        );
        let device = device_row_for(&udid).await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let recorded = reside_core::installs::record_install(
            &state.db,
            &reside_core::installs::InstallRecord {
                device: &device,
                meta: &meta,
                stored_ipa: &stored,
                apple_id: &creds.apple_id,
                team_id: None,
                installed_at: now,
            },
        )
        .await?;

        Ok::<_, AppError>(InstallOutcome {
            installation_id: recorded.installation_id,
            display_name: meta.display_name,
            bundle_id: meta.bundle_id,
            expiration_ts: recorded.expiration_ts,
        })
    };

    match run.await {
        Ok(outcome) => {
            op.done();
            Ok(outcome)
        }
        Err(e @ AppError::AppleAuth2faRequired) => {
            // Not a hard failure — the UI will prompt for the code and retry.
            op.stage(
                OperationStage::Awaiting2fa,
                0.5,
                Some("Waiting for verification code…".into()),
            );
            Err(e.into())
        }
        Err(e) => {
            op.fail(&e);
            Err(e.into())
        }
    }
}

/// Best-effort device identity for the install record: read name/iOS from
/// usbmuxd if the device is reachable, else fall back to just the UDID.
async fn device_row_for(udid: &str) -> reside_core::installs::DeviceRow {
    let found = reside_core::device::list_devices()
        .await
        .ok()
        .and_then(|ds| ds.into_iter().find(|d| d.udid == udid));
    match found {
        Some(d) => reside_core::installs::DeviceRow {
            udid: d.udid,
            name: d.name,
            ios_version: d.ios_version,
        },
        None => reside_core::installs::DeviceRow {
            udid: udid.to_string(),
            name: None,
            ios_version: None,
        },
    }
}

#[derive(serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    installation_id: i64,
    display_name: String,
    bundle_id: String,
    version: Option<String>,
    device_udid: String,
    install_ts: i64,
    expiration_ts: i64,
    refresh_status: String,
}

/// List installed apps (joined with their app metadata), soonest-to-expire
/// first — the Dashboard's live app grid.
#[tauri::command]
async fn list_apps(state: tauri::State<'_, AppState>) -> CmdResult<Vec<InstalledApp>> {
    let rows = sqlx::query_as::<_, InstalledApp>(
        "SELECT i.id AS installation_id, a.display_name, a.bundle_id, a.version, \
                i.device_udid, i.install_ts, i.expiration_ts, i.refresh_status \
         FROM installations i JOIN apps a ON a.id = i.app_id \
         ORDER BY i.expiration_ts ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(AppError::from)?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Auto-refresh (task 11c). A "refresh" re-signs + re-installs the stored IPA
// to reset the free profile's 7-day clock. The engine lives in
// `reside_core::refresh`; these commands are the UI-/agent-facing triggers.
// ---------------------------------------------------------------------------

/// Seconds since the Unix epoch (saturating to 0 before 1970, which never happens).
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshAppOutcome {
    installation_id: i64,
    new_expiration_ts: i64,
}

/// Force a re-sign + re-install of one installation now, regardless of how close
/// it is to expiry — the per-app "Refresh now" button (and the hardware test).
///
/// Progress streams on `operation_{operation_id}`. On a trusted machine this
/// needs no 2FA; if Apple ever demands one it returns `AppleAuth2FARequired`
/// rather than hanging (the same loud-failure contract the agent relies on).
#[tauri::command]
async fn refresh_app(
    state: tauri::State<'_, AppState>,
    operation_id: String,
    installation_id: i64,
) -> CmdResult<RefreshAppOutcome> {
    let op = state.ops.start(operation_id);
    // Use session credentials if the user chose not to persist; else the keyring.
    let session = state.resolve_session_only();
    // refresh_installation emits the terminal Done/Failed event itself.
    let new_expiration_ts = reside_core::refresh::refresh_installation(
        &state.db,
        &state.store,
        Some(&op),
        installation_id,
        unix_now(),
        session.as_ref(),
    )
    .await?;
    Ok(RefreshAppOutcome {
        installation_id,
        new_expiration_ts,
    })
}

/// Refresh every install whose free profile is due (within the lead window),
/// under the single-writer lock. This is exactly what the background agent will
/// call on its timer; the "Refresh all" button invokes it on demand. Raises a
/// desktop notification for anything that needs the user (e.g. re-auth).
#[tauri::command]
async fn refresh_due_now(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> CmdResult<reside_core::refresh::RefreshSummary> {
    let session = state.resolve_session_only();
    let summary = reside_core::refresh::refresh_due(
        &state.db,
        &state.store,
        &state.paths.agent_pid_file(),
        unix_now(),
        reside_core::refresh::REFRESH_LEAD_SECS,
        session.as_ref(),
    )
    .await?;

    for report in summary.needs_attention() {
        notify(
            &app,
            "ReSide couldn't auto-refresh an app",
            &format!(
                "{} needs you to sign in to Apple again to keep it working.",
                report.display_name
            ),
        );
    }
    Ok(summary)
}

/// Fire a desktop notification (best-effort; failures are logged, not surfaced).
fn notify(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(error = %e, "failed to show desktop notification");
    }
}

/// Report whether the background-refresh autopilot is currently enabled, and how
/// it's wired on this host (systemd timer vs. XDG autostart). Backs the Dashboard
/// toggle's initial/refreshed state.
#[tauri::command]
async fn agent_status(
    state: tauri::State<'_, AppState>,
) -> CmdResult<reside_core::refresh::AgentStatus> {
    Ok(reside_core::refresh::agent::status(&state.paths)?)
}

/// Turn the background-refresh autopilot on or off. Enabling installs a
/// `reside-agent` systemd user timer (or an XDG autostart entry on non-systemd
/// hosts) that runs the same `refresh_due` engine on a ~6h schedule even while
/// ReSide is closed; disabling removes it. Returns the resulting status.
#[tauri::command]
async fn set_background_agent(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> CmdResult<reside_core::refresh::AgentStatus> {
    use reside_core::refresh::{agent, AgentConfig};
    let status = if enabled {
        let cfg = AgentConfig::new(
            agent_exec_path()?,
            agent_sideloader_bin(),
            agent_netmuxd_bin(),
        );
        agent::install(&state.paths, &cfg)?
    } else {
        agent::uninstall(&state.paths)?
    };
    Ok(status)
}

/// Locate the headless `reside-agent` binary that the trigger should launch. It
/// ships alongside the app binary (same `target/<profile>/` in dev, same bindir
/// when packaged), so we resolve it relative to the running executable.
fn agent_exec_path() -> Result<std::path::PathBuf, AppError> {
    let exe = std::env::current_exe()
        .map_err(|e| AppError::Internal(format!("cannot locate the ReSide binary: {e}")))?;
    let agent = exe
        .parent()
        .ok_or_else(|| AppError::Internal("ReSide binary has no parent directory".into()))?
        .join("reside-agent");
    if !agent.exists() {
        return Err(AppError::Internal(format!(
            "background agent not found at {} — reside-agent must be installed alongside ReSide",
            agent.display()
        )));
    }
    Ok(agent)
}

/// The Sideloader path to bake into the agent's environment — the app's own
/// resolved signer binary, but only when it's absolute. A bare `sideloader` on
/// `PATH` can't be reproduced in a unit's empty environment, so we leave it unset
/// and let the agent fall back to `PATH` itself.
fn agent_sideloader_bin() -> Option<std::path::PathBuf> {
    let bin = reside_core::signer::sideloader_binary();
    bin.is_absolute().then_some(bin)
}

/// The netmuxd path to bake into the agent's environment, by the same rule as
/// [`agent_sideloader_bin`]: only when absolute, so a unit's empty environment
/// can reproduce it. A bare `netmuxd` on `PATH` is left for the agent to resolve.
fn agent_netmuxd_bin() -> Option<std::path::PathBuf> {
    let bin = reside_core::transport::muxer::netmuxd_binary();
    bin.is_absolute().then_some(bin)
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let paths = Paths::resolve()?;
            paths.ensure_dirs()?;

            let (store, keyring_warning) = SecureStore::detect();
            if keyring_warning.is_some() {
                tracing::warn!(
                    "no system keyring detected — credential storage disabled until one is installed"
                );
            }

            let db = tauri::async_runtime::block_on(reside_core::db::open(paths.database_file()))?;

            // Bridge core operation events → frontend `operation_{id}` events.
            let ops = OperationChannel::new();
            let handle = app.handle().clone();
            let mut rx = ops.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(ev) => {
                            let _ = handle.emit(&format!("operation_{}", ev.id), &ev);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            let ipa_store = IpaStore::new(paths.ipa_store_dir());
            app.manage(AppState {
                paths,
                db,
                store,
                session_creds: std::sync::Mutex::new(None),
                ops,
                tunnels: TunnelManager::new(),
                ipa_store,
            });
            tracing::info!("ReSide core initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_setup_check,
            get_tunnel_status,
            establish_tunnel,
            list_devices,
            pair_device,
            developer_mode_status,
            check_wifi_availability,
            get_activity_log,
            pick_ipa,
            is_signed_in,
            credential_status,
            set_apple_credentials,
            sign_out,
            list_certificates,
            revoke_certificate,
            install_ipa,
            list_apps,
            refresh_app,
            refresh_due_now,
            agent_status,
            set_background_agent
        ])
        .build(tauri::generate_context!())
        .expect("error while building ReSide")
        .run(|_app_handle, event| {
            // Tear down the on-demand Wi-Fi bridge so netmuxd never outlives the
            // app (it's started lazily by Wi-Fi installs/refreshes).
            if let tauri::RunEvent::Exit = event {
                tauri::async_runtime::block_on(reside_core::transport::muxer::shutdown());
            }
        });
}
