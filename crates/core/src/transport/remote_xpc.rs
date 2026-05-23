//! RSD + RemoteXPC transport over USB via the `idevice` crate. Phase 1.
//!
//! iOS 17.4+ developer services live behind an IPv6 RSD tunnel; they cannot be
//! reached over plain usbmux. Over USB the tunnel is established through
//! CoreDeviceProxy (a CDTunnel handshake), then a userspace TCP/IP stack (jktcp)
//! carries connections to the device's RSD port, where RemoteXPC enumerates the
//! services available through the tunnel.
//!
//! Wi-Fi establishment (over `_remotepairing._tcp`) lands in a later slice; this
//! module currently covers the USB path only.
//!
//! Device-dependent: validation requires the user's hardware.

use crate::device::paired_provider;
use crate::error::{AppError, Result};
use idevice::core_device_proxy::CoreDeviceProxy;
use idevice::rsd::RsdHandshake;
use idevice::tcp::handle::AdapterHandle;
use idevice::{IdeviceError, IdeviceService};
use serde::Serialize;

/// A live RSD tunnel to a device. Holds the jktcp adapter handle that owns the
/// background packet loop — dropping the tunnel drops the handle, which shuts
/// the loop down and tears the tunnel down.
pub struct RsdTunnel {
    /// jktcp adapter handle. Also an `RsdProvider`: future slices open service
    /// ports through it. Held here to keep the tunnel alive.
    handle: AdapterHandle,
    /// Network parameters negotiated by the CDTunnel handshake.
    pub endpoint: TunnelEndpoint,
    /// Services advertised by the device over RSD (sorted by name).
    pub services: Vec<DiscoveredService>,
}

/// IPv6 tunnel parameters from the CDTunnel handshake.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelEndpoint {
    /// Device-side IPv6 address reachable through the tunnel.
    pub server_address: String,
    /// Host-side IPv6 address assigned to us by the handshake.
    pub client_address: String,
    /// RSD port on the device, reachable through the tunnel.
    pub rsd_port: u16,
    /// Negotiated MTU.
    pub mtu: u16,
}

/// One service advertised in the RSD handshake.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredService {
    pub name: String,
    pub port: u16,
}

impl RsdTunnel {
    /// Establish an RSD tunnel to a paired device over USB and enumerate the
    /// services reachable through it. Requires a stored pair record.
    pub async fn establish_usb(udid: &str) -> Result<Self> {
        let provider = paired_provider(udid).await?;

        // Open CoreDeviceProxy over the trusted lockdown session and perform the
        // CDTunnel handshake.
        let proxy = CoreDeviceProxy::connect(&provider)
            .await
            .map_err(map_tunnel_error)?;

        let ti = proxy.tunnel_info();
        let endpoint = TunnelEndpoint {
            server_address: ti.server_address.clone(),
            client_address: ti.client_address.clone(),
            rsd_port: ti.server_rsd_port,
            mtu: ti.mtu,
        };

        // Spin up the userspace TCP/IP stack over the tunnel's raw IPv6 packets.
        let adapter = proxy.create_software_tunnel().map_err(map_tunnel_error)?;
        let mut handle = adapter.to_async_handle();

        // Connect to the device's RSD port through the tunnel and read the
        // RemoteXPC service catalogue.
        let rsd_socket = handle.connect(endpoint.rsd_port).await.map_err(|e| {
            tracing::warn!(error = %e, port = endpoint.rsd_port, "connect to RSD port failed");
            AppError::TunnelEstablishFailed
        })?;
        let handshake = RsdHandshake::new(rsd_socket)
            .await
            .map_err(map_tunnel_error)?;

        let mut services: Vec<DiscoveredService> = handshake
            .services
            .iter()
            .map(|(name, s)| DiscoveredService {
                name: name.clone(),
                port: s.port,
            })
            .collect();
        services.sort_by(|a, b| a.name.cmp(&b.name));

        tracing::info!(
            rsd_port = endpoint.rsd_port,
            service_count = services.len(),
            "RSD tunnel established over USB"
        );

        Ok(Self {
            handle,
            endpoint,
            services,
        })
    }

    /// Tear the tunnel down, shutting the background adapter task.
    pub async fn close(mut self) {
        let _ = self.handle.close().await;
    }
}

/// Translate tunnel-establishment failures into the user-facing taxonomy.
fn map_tunnel_error(e: IdeviceError) -> AppError {
    match e {
        IdeviceError::PasswordProtected => AppError::DeviceLocked,
        IdeviceError::InvalidHostID | IdeviceError::UserDeniedPairing => AppError::DeviceNotTrusted,
        other => {
            tracing::warn!(error = %other, "RSD tunnel establishment failed");
            AppError::TunnelEstablishFailed
        }
    }
}
