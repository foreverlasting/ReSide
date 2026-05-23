//! Device detection, pairing, and gating (iOS version + Developer Mode).
//!
//! Phase 1. This slice implements USB/network enumeration over usbmuxd plus a
//! best-effort, pairing-free read of basic lockdown values (name, version).
//! Pairing, Developer Mode state, and RSD tunnels land in subsequent slices.

pub mod pair_record;

use crate::error::{AppError, Result};
use crate::paths::Paths;
use idevice::amfi::AmfiClient;
use idevice::lockdown::LockdownClient;
use idevice::pairing_file::PairingFile;
use idevice::provider::{IdeviceProvider, UsbmuxdProvider};
use idevice::usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdConnection, UsbmuxdDevice};
use idevice::{Idevice, IdeviceError, IdeviceService};
use pair_record::PairRecordStore;
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

/// Enumerate all devices known to usbmuxd (USB + network) and read basic info.
pub async fn list_devices() -> Result<Vec<DeviceInfo>> {
    let mut conn = UsbmuxdConnection::default().await.map_err(|e| {
        tracing::warn!(error = %e, "could not connect to usbmuxd");
        AppError::UsbmuxdDown
    })?;

    let devices = conn.get_devices().await.map_err(|e| {
        tracing::warn!(error = %e, "usbmuxd get_devices failed");
        AppError::UsbmuxdDown
    })?;

    let addr = UsbmuxdAddr::from_env_var().unwrap_or_default();

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
/// persist the resulting record so future sessions are trusted.
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
        .pair(host_id(), buid, Some(HOST_NAME))
        .await
        .map_err(map_pair_error)?;

    let store = PairRecordStore::new(Paths::resolve()?.pair_records_dir());
    store.save(udid, &pairing)?;
    tracing::info!("device paired; pair record stored");
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

/// Build a [`PairedUsbProvider`] for a paired device over USB. Requires a stored
/// pair record (returns [`AppError::DeviceNotTrusted`] otherwise). This is the
/// reusable entry point for every trusted lockdown service and the RSD tunnel —
/// the default `IdeviceService::connect()` flow "just works" with it over USB.
pub(crate) async fn paired_provider(udid: &str) -> Result<PairedUsbProvider> {
    let store = PairRecordStore::new(Paths::resolve()?.pair_records_dir());
    if !store.exists(udid) {
        // Without a trusted session we can't reach trusted services; pair first.
        return Err(AppError::DeviceNotTrusted);
    }
    let pairing_file = store.load(udid)?;

    let mut conn = UsbmuxdConnection::default().await.map_err(|e| {
        tracing::warn!(error = %e, "could not connect to usbmuxd");
        AppError::UsbmuxdDown
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

/// An [`IdeviceProvider`] that connects over USB (delegating to usbmuxd) but
/// supplies ReSide's app-managed pairing record instead of reading usbmuxd's
/// cache (which we deliberately never populate — see [`pair_record`]). Reusable
/// for every trusted lockdown service (amfi, installer, image mounter, …).
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
