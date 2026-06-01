//! Inline RSD tunnel manager. Phase 1.
//!
//! Holds at most one established [`RsdTunnel`] per device UDID, keeping the
//! jktcp adapter task alive for the lifetime of the entry, and reports status
//! to the UI. This is the in-process precursor to the dedicated
//! `reside-tunneld` systemd service of Phase 4 — same responsibilities, no IPC
//! boundary yet.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::Mutex;

use crate::error::{AppError, Result};
use crate::transport::remote_xpc::{DiscoveredService, RsdTunnel, TunnelEndpoint, TunnelTransport};

/// Tracks live tunnels keyed by device UDID. Cloneable: clones share state.
#[derive(Clone, Default)]
pub struct TunnelManager {
    tunnels: Arc<Mutex<HashMap<String, RsdTunnel>>>,
}

/// Status of a device's tunnel, surfaced to the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub udid: String,
    pub connected: bool,
    /// Transport the live tunnel runs over (ROADMAP §7k). Only meaningful when
    /// `connected`; a disconnected status reports `Usb` as a neutral default and
    /// the UI ignores transport unless `connected` is true.
    pub transport: TunnelTransport,
    pub endpoint: Option<TunnelEndpoint>,
    pub services: Vec<DiscoveredService>,
}

impl TunnelStatus {
    fn disconnected(udid: &str) -> Self {
        Self {
            udid: udid.to_string(),
            connected: false,
            transport: TunnelTransport::Usb,
            endpoint: None,
            services: Vec::new(),
        }
    }

    fn from_tunnel(udid: &str, tunnel: &RsdTunnel) -> Self {
        Self {
            udid: udid.to_string(),
            connected: true,
            transport: tunnel.transport,
            endpoint: Some(tunnel.endpoint.clone()),
            services: tunnel.services.clone(),
        }
    }
}

impl TunnelManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Establish (or replace) the tunnel for a device on the requested transport
    /// and return its status (ROADMAP §7k). Replacing drops the prior tunnel,
    /// which closes it.
    ///
    /// Wi-Fi RSD tunnels (idevice `remote_pairing`) are a later slice — see
    /// `remote_xpc`'s module note — so a Wi-Fi request fails cleanly rather than
    /// silently establishing over the cable. The ladder surfaces this as a
    /// "connect via USB" prompt instead of a phantom Wi-Fi tunnel.
    pub async fn connect(&self, udid: &str, wifi: bool) -> Result<TunnelStatus> {
        if wifi {
            tracing::info!(udid, "Wi-Fi tunnel requested; not yet supported (USB only)");
            return Err(AppError::WifiTunnelUnsupported);
        }
        self.connect_usb(udid).await
    }

    /// Establish (or replace) the USB tunnel for a device and return its status.
    /// Replacing drops the prior tunnel, which closes it.
    pub async fn connect_usb(&self, udid: &str) -> Result<TunnelStatus> {
        let tunnel = RsdTunnel::establish_usb(udid).await?;
        let status = TunnelStatus::from_tunnel(udid, &tunnel);
        let mut guard = self.tunnels.lock().await;
        if let Some(prev) = guard.insert(udid.to_string(), tunnel) {
            prev.close().await;
        }
        Ok(status)
    }

    /// Status for a specific device (connected only if we hold a live tunnel).
    pub async fn status(&self, udid: &str) -> TunnelStatus {
        match self.tunnels.lock().await.get(udid) {
            Some(t) => TunnelStatus::from_tunnel(udid, t),
            None => TunnelStatus::disconnected(udid),
        }
    }

    /// True if any device currently has a live tunnel. Backs the aggregate
    /// titlebar indicator, which is not device-scoped.
    pub async fn any_connected(&self) -> bool {
        !self.tunnels.lock().await.is_empty()
    }

    /// Tear down a device's tunnel, if one is held.
    pub async fn disconnect(&self, udid: &str) {
        if let Some(tunnel) = self.tunnels.lock().await.remove(udid) {
            tunnel.close().await;
        }
    }
}
