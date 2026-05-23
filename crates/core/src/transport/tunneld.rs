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

use crate::error::Result;
use crate::transport::remote_xpc::{DiscoveredService, RsdTunnel, TunnelEndpoint};

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
    pub endpoint: Option<TunnelEndpoint>,
    pub services: Vec<DiscoveredService>,
}

impl TunnelStatus {
    fn disconnected(udid: &str) -> Self {
        Self {
            udid: udid.to_string(),
            connected: false,
            endpoint: None,
            services: Vec::new(),
        }
    }

    fn from_tunnel(udid: &str, tunnel: &RsdTunnel) -> Self {
        Self {
            udid: udid.to_string(),
            connected: true,
            endpoint: Some(tunnel.endpoint.clone()),
            services: tunnel.services.clone(),
        }
    }
}

impl TunnelManager {
    pub fn new() -> Self {
        Self::default()
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
