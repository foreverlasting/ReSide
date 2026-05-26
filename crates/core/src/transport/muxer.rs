//! On-demand Wi-Fi transport for the delegated signer. Task 11d.
//!
//! The forked Sideloader reaches the device through libimobiledevice, which on
//! Linux only ever sees a device that `usbmuxd` knows about. Stock `usbmuxd`
//! lists USB devices but never network ones (and on this distro it isn't even
//! running while no cable is attached). So to install/refresh over Wi-Fi we run
//! [`netmuxd`](https://github.com/jkcoxson/netmuxd) — a tiny sidecar that
//! discovers the device over mDNS, reads the *same* `/var/lib/lockdown` pair
//! record ReSide already wrote (see the project-pairing notes), and presents it
//! as a network device on a local TCP muxer port. Pointing the fork at that port
//! via `USBMUXD_SOCKET_ADDRESS` lets the unchanged signer install over Wi-Fi.
//!
//! Lifecycle is **on-demand**: netmuxd starts only when a refresh/install needs
//! Wi-Fi, is reused for the life of the process (so repeated refreshes don't
//! re-pay mDNS discovery), and is torn down by [`shutdown`] — which the app calls
//! on exit and the agent calls after each sweep. The cable path is untouched: if
//! the device is on USB we never start netmuxd at all.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use idevice::usbmuxd::UsbmuxdAddr;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::error::{AppError, Result};

/// TCP address netmuxd listens on in extension mode (its documented default
/// port, alongside the system usbmuxd rather than replacing it).
const NETMUXD_TCP: &str = "127.0.0.1:27015";
/// usbmuxd's pair-record store. netmuxd reads device trust from here — the same
/// directory the system usbmuxd writes on Linux, and where ReSide's own pairing
/// already lands, so no separate Wi-Fi pairing step is needed.
const LOCKDOWN_DIR: &str = "/var/lib/lockdown";
/// Env var overriding the netmuxd binary location (else `netmuxd` on `PATH`).
const ENV_NETMUXD_BIN: &str = "RESIDE_NETMUXD_BIN";

/// How long to wait for netmuxd to discover the device over mDNS before giving
/// up. Cold discovery measured ~38s on hardware, so this leaves headroom.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(75);
/// How often to re-check for the device while waiting for discovery.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// The single netmuxd child this process spawned, if any. `None` means we never
/// started one (Wi-Fi was never needed) or an external netmuxd already serves
/// the port and we reuse it without owning it. Process-global because there can
/// only be one muxer on the port; sharing it across calls is what makes repeated
/// refreshes in one session skip the discovery wait.
fn supervised() -> &'static Mutex<Option<Child>> {
    static CELL: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

/// Resolve the netmuxd binary: `RESIDE_NETMUXD_BIN` if set, else a `netmuxd`
/// shipped next to the running executable, else `netmuxd` on `PATH`. Mirrors how
/// the signer locates the Sideloader binary (both go through [`crate::locate`]).
/// Public so the app can bake the resolved path into the agent's unit
/// environment.
pub fn netmuxd_binary() -> PathBuf {
    crate::locate::helper_binary(ENV_NETMUXD_BIN, "netmuxd")
}

/// The Wi-Fi muxer address as an `idevice` target. `NETMUXD_TCP` is a constant we
/// control, so the parse is infallible.
fn netmuxd_addr() -> UsbmuxdAddr {
    UsbmuxdAddr::TcpSocket(
        NETMUXD_TCP
            .parse::<SocketAddr>()
            .expect("NETMUXD_TCP is a valid socket address"),
    )
}

/// Is `udid` present on the muxer at `addr`? Any connection or protocol error
/// counts as "no" — e.g. the system usbmuxd isn't running (cable out), or
/// netmuxd hasn't discovered the device yet.
async fn device_present(addr: &UsbmuxdAddr, udid: &str) -> bool {
    let Ok(mut conn) = addr.connect(0).await else {
        return false;
    };
    match conn.get_devices().await {
        Ok(devices) => devices.iter().any(|d| d.udid == udid),
        Err(_) => false,
    }
}

/// Decide how the signer should reach `udid`, bringing up the Wi-Fi bridge if
/// needed:
/// - `Ok(None)` — the device is on USB; the signer should use the default muxer.
/// - `Ok(Some(addr))` — reachable over Wi-Fi; point the signer at `addr` via
///   `USBMUXD_SOCKET_ADDRESS`.
/// - `Err(DeviceOffline)` — not on USB and not discoverable over Wi-Fi.
pub async fn route_to(udid: &str) -> Result<Option<String>> {
    // 1. Cable wins: if the system usbmuxd has the device, use USB unchanged.
    //    `default()` targets the real unix socket regardless of any
    //    `USBMUXD_SOCKET_ADDRESS` in our own environment, so this check always
    //    reflects a genuine USB attachment.
    if device_present(&UsbmuxdAddr::default(), udid).await {
        tracing::debug!(udid, "device reachable over USB; using the cable");
        return Ok(None);
    }

    let wifi = netmuxd_addr();

    // 2. A netmuxd is already serving the device (one we started earlier this
    //    session, or an external one) — reuse it; never run two on one port.
    if device_present(&wifi, udid).await {
        tracing::debug!(udid, "device already reachable over Wi-Fi via netmuxd");
        return Ok(Some(NETMUXD_TCP.to_string()));
    }

    // 3. Start netmuxd (unless something is already on the port) and wait for it
    //    to discover the device over mDNS.
    ensure_netmuxd().await?;
    if wait_for_device(&wifi, udid).await {
        tracing::info!(udid, "device reachable over Wi-Fi via netmuxd");
        return Ok(Some(NETMUXD_TCP.to_string()));
    }

    tracing::warn!(udid, "device not reachable over USB or Wi-Fi");
    Err(AppError::DeviceOffline)
}

/// Ensure a netmuxd is listening on the muxer port, spawning one we own if not.
/// Reuses an already-listening instance (ours from earlier, or external) so a
/// second process never collides on the port.
async fn ensure_netmuxd() -> Result<()> {
    let wifi = netmuxd_addr();
    if wifi.connect(0).await.is_ok() {
        return Ok(());
    }
    let mut guard = supervised().lock().await;
    // Re-check under the lock: another task may have started it meanwhile.
    if wifi.connect(0).await.is_ok() {
        return Ok(());
    }
    let child = spawn_netmuxd()?;
    // Replaces any dead child we previously held; dropping it kills that one.
    *guard = Some(child);
    Ok(())
}

/// Spawn netmuxd in extension mode: a local TCP muxer that adds network devices
/// alongside the system usbmuxd, trusting the shared `/var/lib/lockdown` records.
/// `kill_on_drop` reaps it if the owning handle is dropped without [`shutdown`].
fn spawn_netmuxd() -> Result<Child> {
    let bin = netmuxd_binary();
    tracing::info!(bin = %bin.display(), "starting netmuxd Wi-Fi bridge");
    Command::new(&bin)
        .arg("--disable-unix")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--plist-storage")
        .arg(LOCKDOWN_DIR)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            AppError::Internal(format!(
                "could not start the Wi-Fi bridge (netmuxd at {}): {e}",
                bin.display()
            ))
        })
}

/// Poll the muxer until the device appears or [`DISCOVERY_TIMEOUT`] elapses.
async fn wait_for_device(addr: &UsbmuxdAddr, udid: &str) -> bool {
    let deadline = tokio::time::Instant::now() + DISCOVERY_TIMEOUT;
    loop {
        if device_present(addr, udid).await {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Stop the netmuxd this process started, if any. Idempotent and safe to call
/// when none is running. The app calls this on exit; the agent after each sweep,
/// so an on-demand bridge never lingers past the work that needed it.
pub async fn shutdown() {
    if let Some(mut child) = supervised().lock().await.take() {
        tracing::info!("stopping netmuxd Wi-Fi bridge");
        let _ = child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_path_honors_env_override() {
        std::env::set_var(ENV_NETMUXD_BIN, "/opt/custom/netmuxd");
        assert_eq!(netmuxd_binary(), PathBuf::from("/opt/custom/netmuxd"));
        std::env::remove_var(ENV_NETMUXD_BIN);
        assert_eq!(netmuxd_binary(), PathBuf::from("netmuxd"));
    }

    #[test]
    fn netmuxd_addr_parses_to_loopback_default_port() {
        match netmuxd_addr() {
            UsbmuxdAddr::TcpSocket(addr) => {
                assert!(addr.ip().is_loopback());
                assert_eq!(addr.port(), 27015);
            }
            #[allow(unreachable_patterns)]
            other => panic!("expected a TCP socket address, got {other:?}"),
        }
    }
}
