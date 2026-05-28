//! Device detection, pairing, and gating (iOS version + Developer Mode).
//!
//! Phase 1. This slice implements USB/network enumeration over usbmuxd plus a
//! best-effort, pairing-free read of basic lockdown values (name, version).
//! Pairing, Developer Mode state, and RSD tunnels land in subsequent slices.

use crate::error::{AppError, Result};
use idevice::amfi::AmfiClient;
use idevice::lockdown::LockdownClient;
use idevice::pairing_file::PairingFile;
use idevice::provider::{IdeviceProvider, UsbmuxdProvider};
use idevice::usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdConnection, UsbmuxdDevice};
use idevice::{Idevice, IdeviceError, IdeviceService};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::pin::Pin;

/// Minimum iOS/iPadOS version ReSide supports (RemoteXPC era). See plan.md §iOS Scope.
const MIN_IOS_MAJOR: u32 = 17;
const MIN_IOS_MINOR: u32 = 4;

const LABEL: &str = "reside";

/// Host name sent to the device in the pairing record; shows up nowhere
/// user-facing but identifies this computer in the device's trust store.
const HOST_NAME: &str = "ReSide";

/// A device as surfaced to the UI. Basic lockdown fields are best-effort: an
/// unpaired or busy device still appears (by UDID + connection) with `null`
/// info rather than vanishing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub udid: String,
    pub name: Option<String>,
    pub ios_version: Option<String>,
    pub product_type: Option<String>,
    /// "usb" | "network" | other (raw description).
    pub connection: String,
    pub wifi: bool,
    /// True when iOS ≥ 17.4 (or unknown — we don't reject what we can't read).
    pub supported: bool,
}

/// Enumerate devices visible to ReSide and read basic info for each.
///
/// Returns USB devices from the system `usbmuxd` merged with any Wi-Fi devices
/// the on-demand bridge resolved earlier this session (stored in
/// [`crate::transport::wifi_cache`]). USB wins when the same UDID appears on
/// both sides — a phone plugged in mid-session should never render twice. The
/// cache exists because the Wi-Fi bridge (`transport::muxer`) is deliberately
/// torn down after each use, so without it Wi-Fi cards would vanish.
pub async fn list_devices() -> Result<Vec<DeviceInfo>> {
    // USB door: the unix socket, ignoring `USBMUXD_SOCKET_ADDRESS` so a stray
    // env var from a concurrent Wi-Fi install can never redirect us. On Linux
    // `usbmuxd` is **udev-activated** — it EXITS when no cable is attached, so
    // a connect failure here is the normal "nothing on USB" state, not an
    // error. Degrade to an empty list and keep going; the Wi-Fi cache below
    // still gets to surface a resolved device.
    let mut out = match enumerate_devices_at(UsbmuxdAddr::default()).await {
        Ok(list) => list,
        Err(e) => {
            tracing::debug!(error = %e, "usbmuxd unavailable; treating as no-USB-devices");
            Vec::new()
        }
    };

    // Wi-Fi door: in-memory snapshot of whatever a prior `resolve_wifi_devices`
    // populated. No bridge process is running right now; this is pure data.
    for cached in crate::transport::wifi_cache::get().await {
        if !out.iter().any(|d| d.udid == cached.udid) {
            out.push(cached);
        }
    }
    Ok(out)
}

/// Enumerate devices visible through a specific muxer (the system unix socket
/// for USB, netmuxd's TCP socket for Wi-Fi). Reused by `list_devices` and by
/// the Wi-Fi resolve path in `transport::muxer`.
pub async fn enumerate_devices_at(addr: UsbmuxdAddr) -> Result<Vec<DeviceInfo>> {
    let mut conn = addr.connect(0).await.map_err(|e| {
        tracing::warn!(error = %e, ?addr, "could not connect to muxer");
        AppError::UsbmuxdDown
    })?;

    let devices = conn.get_devices().await.map_err(|e| {
        tracing::warn!(error = %e, ?addr, "muxer get_devices failed");
        AppError::UsbmuxdDown
    })?;

    let mut out = Vec::with_capacity(devices.len());
    for d in devices {
        let (connection, wifi) = match &d.connection_type {
            Connection::Usb => ("usb".to_string(), false),
            Connection::Network(_) => ("network".to_string(), true),
            Connection::Unknown(s) => (s.clone(), false),
        };

        let (name, ios_version, product_type) = read_basic_info(&d, addr.clone()).await;
        let supported = ios_version
            .as_deref()
            .map(is_supported_version)
            .unwrap_or(true);

        out.push(DeviceInfo {
            udid: d.udid,
            name,
            ios_version,
            product_type,
            connection,
            wifi,
            supported,
        });
    }
    Ok(out)
}

/// Best-effort lockdown read of name/version. Requires no pairing for these
/// public values; returns `None`s if the device is unreachable or refuses.
async fn read_basic_info(
    device: &UsbmuxdDevice,
    addr: UsbmuxdAddr,
) -> (Option<String>, Option<String>, Option<String>) {
    let provider = device.to_provider(addr, LABEL);
    let mut lockdown = match LockdownClient::connect(&provider).await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(error = %e, "lockdown connect failed (device may be locked/unpaired)");
            return (None, None, None);
        }
    };
    let name = get_string(&mut lockdown, "DeviceName").await;
    let version = get_string(&mut lockdown, "ProductVersion").await;
    let product = get_string(&mut lockdown, "ProductType").await;
    (name, version, product)
}

async fn get_string(client: &mut LockdownClient, key: &str) -> Option<String> {
    client
        .get_value(Some(key), None)
        .await
        .ok()
        .and_then(|v| v.as_string().map(str::to_string))
}

/// Pair with a device over USB: generate a pairing record, send it to the
/// device (which surfaces the on-screen "Trust This Computer?" dialog), and
/// hand the resulting record to usbmuxd so the whole host trusts the device.
///
/// ReSide keeps **no private pairing store of its own**. It saves the record
/// into usbmuxd via the same `SavePairRecord` that `idevicepair pair` performs,
/// so usbmuxd is the single source of truth shared by ReSide and the
/// `libimobiledevice` path the delegated signer (Sideloader) uses. This is what
/// stops ReSide's Pair button from clobbering the signer's trust — both sides
/// now read and write the one record. See the project-pairing-dual-stack notes.
///
/// This blocks until the user responds to the dialog — `idevice`'s `pair()`
/// polls while the response is pending. Validation requires real hardware.
pub async fn pair_device(udid: &str) -> Result<()> {
    let mut conn = UsbmuxdConnection::default().await.map_err(|e| {
        tracing::warn!(error = %e, "could not connect to usbmuxd for pairing");
        AppError::UsbmuxdDown
    })?;

    // BUID identifies this host's usbmuxd install; it goes into the pair record.
    let buid = conn.get_buid().await.map_err(|e| {
        tracing::warn!(error = %e, "usbmuxd ReadBUID failed");
        AppError::UsbmuxdDown
    })?;

    // Reuse the HostID usbmuxd already holds for this device (left by an earlier
    // `idevicepair pair` or a prior ReSide pairing) so ReSide and
    // libimobiledevice keep one shared identity and never churn the device's
    // trust. Only on a first-ever pairing do we mint a stable, machine-derived
    // id. Any read error just means "no record yet" — fall back to minting.
    let host_id = match conn.get_pair_record(udid).await {
        Ok(existing) => existing.host_id,
        Err(_) => host_id(),
    };

    let device = conn.get_device(udid).await.map_err(|e| {
        tracing::warn!(error = %e, "device not present in usbmuxd");
        AppError::DeviceOffline
    })?;

    let addr = UsbmuxdAddr::from_env_var().unwrap_or_default();
    let provider = device.to_provider(addr, LABEL);
    let mut lockdown = LockdownClient::connect(&provider).await.map_err(|e| {
        tracing::warn!(error = %e, "lockdown connect for pairing failed");
        AppError::DeviceOffline
    })?;

    let pairing = lockdown
        .pair(host_id, buid, Some(HOST_NAME))
        .await
        .map_err(map_pair_error)?;

    // Persist into usbmuxd's own store (on Linux this lands in
    // /var/lib/lockdown/<udid>.plist, written by usbmuxd itself — no root needed
    // on our side). libimobiledevice validates using the HostID in this record,
    // so the device just accepted exactly what the signer will present.
    let bytes = pairing
        .serialize()
        .map_err(|e| AppError::Internal(format!("serialize pairing file: {e}")))?;
    conn.save_pair_record(udid, bytes).await.map_err(|e| {
        tracing::warn!(error = %e, "usbmuxd SavePairRecord failed");
        AppError::Internal("could not save pair record to usbmuxd".into())
    })?;

    tracing::info!("device paired; pair record saved to usbmuxd");
    Ok(())
}

/// Translate `idevice` pairing failures into the user-facing taxonomy.
fn map_pair_error(e: idevice::IdeviceError) -> AppError {
    use idevice::IdeviceError as E;
    match e {
        // User tapped "Don't Trust" (or dismissed the dialog).
        E::UserDeniedPairing => AppError::DeviceNotTrusted,
        // Device is locked with a passcode; pairing can't proceed.
        E::PasswordProtected => AppError::DeviceLocked,
        other => {
            tracing::warn!(error = %other, "pairing failed");
            AppError::DeviceNotTrusted
        }
    }
}

/// Read whether Developer Mode is enabled on a device. iOS 16+ requires it for
/// any developer/install flow; v1 targets 17.4+, so this is unconditional (see
/// plan.md §iOS Scope). Talks to the trusted `com.apple.amfi.lockdown` service,
/// so a stored pair record is required — pair the device first.
///
/// Device-dependent: validation requires the user's hardware.
pub async fn developer_mode_status(udid: &str) -> Result<bool> {
    let provider = paired_provider(udid).await?;
    let mut amfi = AmfiClient::connect(&provider)
        .await
        .map_err(map_service_error)?;
    amfi.get_developer_mode_status()
        .await
        .map_err(map_service_error)
}

/// Build a [`PairedUsbProvider`] for a paired device over USB. Reads the pairing
/// record straight from usbmuxd — ReSide's single source of trust — so it works
/// whether the device was paired via ReSide's Pair button or `idevicepair pair`.
/// A missing record yields [`AppError::DeviceNotTrusted`] (pair first). This is
/// the reusable entry point for every trusted lockdown service and the RSD
/// tunnel — the default `IdeviceService::connect()` flow "just works" over USB.
pub(crate) async fn paired_provider(udid: &str) -> Result<PairedUsbProvider> {
    let mut conn = UsbmuxdConnection::default().await.map_err(|e| {
        tracing::warn!(error = %e, "could not connect to usbmuxd");
        AppError::UsbmuxdDown
    })?;

    // No record means this host has never been trusted by the device (neither
    // ReSide nor `idevicepair pair` has run) — surface as "pair first".
    let pairing_file = conn.get_pair_record(udid).await.map_err(|e| {
        tracing::debug!(error = %e, "no usbmuxd pair record for device");
        AppError::DeviceNotTrusted
    })?;

    let device = conn.get_device(udid).await.map_err(|e| {
        tracing::warn!(error = %e, "device not present in usbmuxd");
        AppError::DeviceOffline
    })?;

    let addr = UsbmuxdAddr::from_env_var().unwrap_or_default();
    Ok(PairedUsbProvider {
        inner: device.to_provider(addr, LABEL),
        pairing_file,
    })
}

/// Translate trusted-service connect/read failures into the user-facing taxonomy.
fn map_service_error(e: IdeviceError) -> AppError {
    match e {
        IdeviceError::PasswordProtected => AppError::DeviceLocked,
        IdeviceError::InvalidHostID | IdeviceError::UserDeniedPairing => AppError::DeviceNotTrusted,
        other => {
            tracing::warn!(error = %other, "trusted service call failed");
            AppError::DeviceOffline
        }
    }
}

/// An [`IdeviceProvider`] that connects over USB (delegating to usbmuxd) and
/// supplies the pairing record read back from usbmuxd. Reusable for every
/// trusted lockdown service (amfi, installer, image mounter, …).
#[derive(Debug)]
pub(crate) struct PairedUsbProvider {
    inner: UsbmuxdProvider,
    pairing_file: PairingFile,
}

impl IdeviceProvider for PairedUsbProvider {
    fn connect(
        &self,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<Idevice, IdeviceError>> + Send>> {
        self.inner.connect(port)
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn get_pairing_file(
        &self,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<PairingFile, IdeviceError>> + Send>> {
        let pairing_file = self.pairing_file.clone();
        Box::pin(async move { Ok(pairing_file) })
    }
}

/// A stable, per-install host identifier in UUID form. Derived deterministically
/// from `/etc/machine-id` so it survives restarts without being persisted and is
/// unique per machine — never copy it (or pair records) between machines.
fn host_id() -> String {
    let seed = std::fs::read_to_string("/etc/machine-id")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "reside-default-host".to_string());

    let mut hasher = Sha256::new();
    hasher.update(b"reside-host-id:");
    hasher.update(seed.as_bytes());
    let d = hasher.finalize();

    format!(
        "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8], d[9], d[10], d[11], d[12], d[13], d[14], d[15]
    )
}

/// True if `version` (e.g. "17.4.1") is ≥ 17.4.
fn is_supported_version(version: &str) -> bool {
    let mut parts = version.split('.').filter_map(|p| p.parse::<u32>().ok());
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    major > MIN_IOS_MAJOR || (major == MIN_IOS_MAJOR && minor >= MIN_IOS_MINOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_gate_matches_plan_scope() {
        assert!(is_supported_version("17.4"));
        assert!(is_supported_version("17.4.1"));
        assert!(is_supported_version("18.0"));
        assert!(is_supported_version("17.10"));
        assert!(!is_supported_version("17.3.1"));
        assert!(!is_supported_version("16.7"));
        assert!(!is_supported_version("15.0"));
    }

    fn fake_device(udid: &str, wifi: bool) -> DeviceInfo {
        DeviceInfo {
            udid: udid.to_string(),
            name: Some(format!("Phone {udid}")),
            ios_version: Some("17.4".into()),
            product_type: Some("iPhone14,5".into()),
            connection: if wifi { "network".into() } else { "usb".into() },
            wifi,
            supported: true,
        }
    }

    /// The merge in `list_devices` must keep the USB entry when the same UDID
    /// appears in both lists — the same physical phone should never render
    /// twice if it's both plugged in and on Wi-Fi.
    #[tokio::test]
    async fn wifi_cache_does_not_double_render_a_usb_device() {
        crate::transport::wifi_cache::set(vec![fake_device("AAA", true), fake_device("CCC", true)])
            .await;

        // Pretend we got AAA on USB and the cache also has it on Wi-Fi.
        let usb = vec![fake_device("AAA", false), fake_device("BBB", false)];
        let mut out = usb;
        for cached in crate::transport::wifi_cache::get().await {
            if !out.iter().any(|d| d.udid == cached.udid) {
                out.push(cached);
            }
        }

        assert_eq!(out.len(), 3, "AAA must not appear twice");
        // The USB row for AAA wins (wifi flag false).
        assert!(!out.iter().find(|d| d.udid == "AAA").unwrap().wifi);
        // CCC came from the cache, BBB from USB — both still present.
        assert!(out.iter().any(|d| d.udid == "BBB"));
        assert!(out.iter().any(|d| d.udid == "CCC"));

        crate::transport::wifi_cache::clear().await;
    }

    #[test]
    fn host_id_is_stable_and_uuid_shaped() {
        let a = host_id();
        let b = host_id();
        assert_eq!(a, b, "host id must be deterministic within an install");
        assert_eq!(a.len(), 36);
        assert_eq!(a.matches('-').count(), 4);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }
}
