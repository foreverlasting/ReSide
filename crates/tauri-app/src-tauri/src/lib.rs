//! ReSide Tauri shell. Commands are thin shims over `reside-core`; this layer
//! owns IPC, the operation-event bridge, and redaction of serialized payloads.

pub mod redaction;

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
    ops: OperationChannel,
    tunnels: TunnelManager,
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

            let (store, keyring_warning) = SecureStore::detect(paths.data_dir().join("secrets"));
            if keyring_warning.is_some() {
                tracing::warn!("no system keyring detected — using filesystem fallback");
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

            app.manage(AppState {
                paths,
                db,
                store,
                ops,
                tunnels: TunnelManager::new(),
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
            get_activity_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running ReSide");
}
