//! mDNS discovery of `_remoted._tcp` / `_remotepairing._tcp` for Wi-Fi
//! reachability. Phase 1.
//!
//! iOS 17.4+ devices advertise `_remoted._tcp` (the RSD endpoint) and
//! `_remotepairing._tcp` (the Wi-Fi pairing endpoint) over mDNS while they are
//! reachable on the local network. Discovering either is the pre-tunnel signal
//! that a paired device can be refreshed over Wi-Fi.
//!
//! This slice answers "is any RemoteXPC-capable device reachable on this
//! network?" Mapping an endpoint to a specific UDID requires establishing the
//! Wi-Fi tunnel and an RSD handshake, which lands with the Wi-Fi tunnel slice.
//!
//! Network-dependent: validation requires the user's hardware on Wi-Fi.

use std::collections::BTreeMap;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;

use crate::error::{AppError, Result};

/// mDNS service types that signal an iOS device is reachable over Wi-Fi.
///
/// `_apple-mobdev2._tcp` is the one iOS actually advertises over Wi-Fi (verified
/// on hardware): its instance name carries a `supportsRP-NN` suffix on iOS 17.4+,
/// meaning the device supports Remote Pairing (RemoteXPC) — the prerequisite for
/// Wi-Fi refresh. `_remoted._tcp` (RSD) and `_remotepairing._tcp` are
/// RemoteXPC-era types that surface over the USB-ethernet interface or once a
/// tunnel / remote pairing is active; we browse them too so they're picked up
/// when present.
const SERVICE_TYPES: [&str; 3] = [
    "_apple-mobdev2._tcp.local.",
    "_remotepairing._tcp.local.",
    "_remoted._tcp.local.",
];

/// How long to listen for mDNS responses. Responders on a LAN answer within a
/// second; we wait a little longer to catch slow ones without hanging the UI.
const BROWSE_WINDOW: Duration = Duration::from_secs(3);

/// A RemoteXPC-capable endpoint discovered on the local network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiEndpoint {
    /// The mDNS service type it was advertised under (trailing dot stripped).
    pub service_type: String,
    /// Advertised host name (trailing dot stripped).
    pub host: String,
    /// Resolved IP addresses (v4 + v6), sorted for stable output.
    pub addresses: Vec<String>,
    pub port: u16,
}

/// Result of a Wi-Fi reachability scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiAvailability {
    /// True when at least one RemoteXPC endpoint was discovered.
    pub available: bool,
    pub endpoints: Vec<WifiEndpoint>,
}

/// Browse the local network for RemoteXPC endpoints over a fixed window.
pub async fn check_wifi_availability() -> Result<WifiAvailability> {
    discover(BROWSE_WINDOW).await
}

async fn discover(window: Duration) -> Result<WifiAvailability> {
    let daemon = ServiceDaemon::new().map_err(|e| {
        tracing::warn!(error = %e, "failed to start mDNS daemon");
        AppError::WifiUnreachable
    })?;

    // Browse every type at once; collecting them one-at-a-time would multiply
    // the wait by the number of types.
    let mut receivers = Vec::new();
    for service_type in SERVICE_TYPES {
        match daemon.browse(service_type) {
            Ok(rx) => receivers.push((service_type, rx)),
            Err(e) => tracing::warn!(error = %e, service_type, "mDNS browse failed"),
        }
    }

    // Key by mDNS fullname so repeated announcements of the same instance dedup.
    let mut found: BTreeMap<String, WifiEndpoint> = BTreeMap::new();
    let deadline = tokio::time::Instant::now() + window;
    while tokio::time::Instant::now() < deadline {
        let mut idle = true;
        for (service_type, rx) in &receivers {
            // Drain everything currently queued without blocking the runtime.
            while let Ok(event) = rx.try_recv() {
                idle = false;
                // Only resolved events carry addresses + port.
                if let ServiceEvent::ServiceResolved(info) = event {
                    found.insert(
                        info.get_fullname().to_string(),
                        endpoint_from(service_type, &info),
                    );
                }
            }
        }
        if idle {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    // Best-effort: the daemon also stops once the last handle drops.
    let _ = daemon.shutdown();

    let endpoints: Vec<WifiEndpoint> = found.into_values().collect();
    Ok(WifiAvailability {
        available: !endpoints.is_empty(),
        endpoints,
    })
}

fn endpoint_from(service_type: &str, info: &ServiceInfo) -> WifiEndpoint {
    let mut addresses: Vec<String> = info.get_addresses().iter().map(|a| a.to_string()).collect();
    addresses.sort();
    WifiEndpoint {
        service_type: service_type
            .trim_end_matches('.')
            .trim_end_matches(".local")
            .to_string(),
        host: info.get_hostname().trim_end_matches('.').to_string(),
        addresses,
        port: info.get_port(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_from_strips_dots_and_sorts_addresses() {
        let info = ServiceInfo::new(
            "_remoted._tcp.local.",
            "Orange Crush",
            "orange-crush.local.",
            "192.168.1.20,192.168.1.5",
            49152,
            &[] as &[(&str, &str)],
        )
        .expect("valid service info");

        let ep = endpoint_from("_remoted._tcp.local.", &info);

        assert_eq!(ep.service_type, "_remoted._tcp");
        assert_eq!(ep.host, "orange-crush.local");
        assert_eq!(ep.port, 49152);
        // Addresses are sorted, so order is deterministic regardless of input.
        assert_eq!(ep.addresses, vec!["192.168.1.20", "192.168.1.5"]);
    }

    #[test]
    fn empty_scan_reports_unavailable() {
        let avail = WifiAvailability {
            available: false,
            endpoints: Vec::new(),
        };
        let json = serde_json::to_string(&avail).unwrap();
        assert!(json.contains("\"available\":false"));
        assert!(json.contains("\"endpoints\":[]"));
    }
}
