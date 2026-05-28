//! Session cache of Wi-Fi-resolved devices. Process-global, in-memory only.
//!
//! The on-demand Wi-Fi bridge (`transport::muxer`) is deliberately torn down
//! after each use, so any device netmuxd discovered for us would otherwise
//! disappear from the Dashboard rail the moment the bridge stops. To keep the
//! resolved card visible while respecting the "no standing netmuxd" contract,
//! [`resolve_wifi_devices`](super::muxer::resolve_wifi_devices) stores the
//! `DeviceInfo` it read here right before it calls
//! [`shutdown`](super::muxer::shutdown). [`crate::device::list_devices`] then
//! merges this cache with the USB list it gets from the system usbmuxd, so the
//! UI sees the Wi-Fi phone as a full named card alongside any cabled ones.
//!
//! Nothing here touches disk: a process restart wipes the cache, so the rail
//! starts in its empty-with-banner state on every launch — that's the design.
//! The honesty win is intentional: a persisted card could lie about a phone
//! that's now off, off-network, or unpaired since.

use std::sync::OnceLock;
use tokio::sync::Mutex;

use crate::device::DeviceInfo;

fn cache() -> &'static Mutex<Vec<DeviceInfo>> {
    static CELL: OnceLock<Mutex<Vec<DeviceInfo>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replace the cached set with `devices`. Pass an empty `Vec` to clear.
pub async fn set(devices: Vec<DeviceInfo>) {
    *cache().lock().await = devices;
}

/// Snapshot the cached entries. Empty vec when nothing has been resolved this session.
pub async fn get() -> Vec<DeviceInfo> {
    cache().lock().await.clone()
}

/// Drop every cached entry. Convenience for tests / manual disconnect.
pub async fn clear() {
    cache().lock().await.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(udid: &str) -> DeviceInfo {
        DeviceInfo {
            udid: udid.to_string(),
            name: Some(format!("Phone {udid}")),
            ios_version: Some("17.4".into()),
            product_type: Some("iPhone14,5".into()),
            connection: "network".into(),
            wifi: true,
            supported: true,
        }
    }

    #[tokio::test]
    async fn set_then_get_roundtrips() {
        clear().await;
        set(vec![sample("AAA"), sample("BBB")]).await;
        let got = get().await;
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].udid, "AAA");
        clear().await;
        assert!(get().await.is_empty());
    }
}
