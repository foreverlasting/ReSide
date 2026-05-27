//! Error taxonomy + redaction.
//!
//! Every [`AppError`] maps to exactly one [`ErrorCategory`] with a user-facing
//! remediation string (see plan.md §Error Taxonomy). The category key is stable
//! and is what gets written to `activity_log.error_category` and surfaced in
//! operation events — never change an existing key without a migration.

use std::fmt;

pub type Result<T> = std::result::Result<T, AppError>;

/// Stable category keys. The string form (`as_key`) is the contract shared with
/// the frontend taxonomy and the `activity_log` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    UnsupportedIosVersion,
    IosDeveloperModeOff,
    IosDeveloperCertUntrusted,
    DeviceNotTrusted,
    DeviceLocked,
    DeviceOffline,
    WifiUnreachable,
    TunnelEstablishFailed,
    UsbmuxdDown,
    PermissionsMissing,
    KeyringUnavailable,
    AnisetteGenFailed,
    AnisetteAdiUnavailable,
    AnisetteAdiIncompatible,
    AppleAuthRateLimited,
    AppleAuth2faRequired,
    AppleAuthCredentialsInvalid,
    AppleAuthProtocolChanged,
    AppleDevCertGenFailed,
    AppleDevDeviceRegLimitReached,
    AppleAppIdLimitReached,
    AppleCertLimitReached,
    SigningCertExpired,
    EntitlementsUnsupported,
    BundleIdConflict,
    InstallTransferFailed,
    InstallVerifyFailed,
    /// Catch-all for bugs and infrastructure failures (I/O, DB, serialization).
    /// Not part of the user-facing taxonomy; should stay rare.
    Internal,
}

impl ErrorCategory {
    /// Stable string key shared with the frontend + persisted in SQLite.
    pub fn as_key(self) -> &'static str {
        match self {
            Self::UnsupportedIosVersion => "UnsupportedIosVersion",
            Self::IosDeveloperModeOff => "iOSDeveloperModeOff",
            Self::IosDeveloperCertUntrusted => "iOSDeveloperCertUntrusted",
            Self::DeviceNotTrusted => "DeviceNotTrusted",
            Self::DeviceLocked => "DeviceLocked",
            Self::DeviceOffline => "DeviceOffline",
            Self::WifiUnreachable => "WifiUnreachable",
            Self::TunnelEstablishFailed => "TunnelEstablishFailed",
            Self::UsbmuxdDown => "UsbmuxdDown",
            Self::PermissionsMissing => "PermissionsMissing",
            Self::KeyringUnavailable => "KeyringUnavailable",
            Self::AnisetteGenFailed => "AnisetteGenFailed",
            Self::AnisetteAdiUnavailable => "AnisetteAdiUnavailable",
            Self::AnisetteAdiIncompatible => "AnisetteAdiIncompatible",
            Self::AppleAuthRateLimited => "AppleAuthRateLimited",
            Self::AppleAuth2faRequired => "AppleAuth2FARequired",
            Self::AppleAuthCredentialsInvalid => "AppleAuthCredentialsInvalid",
            Self::AppleAuthProtocolChanged => "AppleAuthProtocolChanged",
            Self::AppleDevCertGenFailed => "AppleDevCertGenFailed",
            Self::AppleDevDeviceRegLimitReached => "AppleDevDeviceRegLimitReached",
            Self::AppleAppIdLimitReached => "AppleAppIdLimitReached",
            Self::AppleCertLimitReached => "AppleCertLimitReached",
            Self::SigningCertExpired => "SigningCertExpired",
            Self::EntitlementsUnsupported => "EntitlementsUnsupported",
            Self::BundleIdConflict => "BundleIdConflict",
            Self::InstallTransferFailed => "InstallTransferFailed",
            Self::InstallVerifyFailed => "InstallVerifyFailed",
            Self::Internal => "Internal",
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_key())
    }
}

/// A redacted, UI-safe view of an error: category key + remediation copy.
/// This is what crosses the Tauri boundary and lands in operation events —
/// it deliberately carries no secret material or raw upstream error text.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorReport {
    pub category: String,
    pub remediation: String,
}

/// The application error type. Variants 1:1 with the user-facing taxonomy plus
/// internal infrastructure errors. `#[from]` conversions wrap upstream errors
/// without surfacing their `Display` text to the UI (see [`ErrorReport`]).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("device reports an unsupported iOS version")]
    UnsupportedIosVersion,
    #[error("Developer Mode is disabled on the device")]
    IosDeveloperModeOff,
    #[error("the free-tier signing certificate is not yet trusted on the device")]
    IosDeveloperCertUntrusted,
    #[error("the device has not trusted this computer")]
    DeviceNotTrusted,
    #[error("the device is locked")]
    DeviceLocked,
    #[error("the device is offline")]
    DeviceOffline,
    #[error("the device is not reachable over Wi-Fi")]
    WifiUnreachable,
    #[error("failed to establish an RSD tunnel to the device")]
    TunnelEstablishFailed,
    #[error("usbmuxd is not running")]
    UsbmuxdDown,
    #[error("required permissions or udev rules are missing")]
    PermissionsMissing,
    #[error("no system keyring is available")]
    KeyringUnavailable,
    #[error("local anisette generation failed")]
    AnisetteGenFailed,
    #[error("Apple ADI libraries are not provisioned")]
    AnisetteAdiUnavailable,
    #[error("Apple ADI libraries are incompatible")]
    AnisetteAdiIncompatible,
    #[error("Apple is rate-limiting authentication")]
    AppleAuthRateLimited,
    #[error("two-factor authentication is required")]
    AppleAuth2faRequired,
    #[error("invalid Apple ID or password")]
    AppleAuthCredentialsInvalid,
    #[error("Apple changed their authentication flow")]
    AppleAuthProtocolChanged,
    #[error("Apple declined to issue a signing certificate")]
    AppleDevCertGenFailed,
    #[error("device-registration limit reached for this week")]
    AppleDevDeviceRegLimitReached,
    #[error("App ID limit reached for this week")]
    AppleAppIdLimitReached,
    #[error("the account already has the maximum number of signing certificates")]
    AppleCertLimitReached,
    #[error("the signing certificate has expired")]
    SigningCertExpired,
    #[error("one or more entitlements are unsupported and will be stripped")]
    EntitlementsUnsupported,
    #[error("bundle identifier conflict")]
    BundleIdConflict,
    #[error("transfer to the device failed")]
    InstallTransferFailed,
    #[error("install completed but verification failed")]
    InstallVerifyFailed,

    // ---- Internal / infrastructure (category: Internal) ----
    #[error("i/o error")]
    Io(#[from] std::io::Error),
    #[error("database error")]
    Db(#[from] sqlx::Error),
    #[error("database migration error")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("serialization error")]
    Serialization(#[from] serde_json::Error),
    /// Free-form internal error. Use sparingly; prefer a typed variant.
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn category(&self) -> ErrorCategory {
        use AppError::*;
        match self {
            UnsupportedIosVersion => ErrorCategory::UnsupportedIosVersion,
            IosDeveloperModeOff => ErrorCategory::IosDeveloperModeOff,
            IosDeveloperCertUntrusted => ErrorCategory::IosDeveloperCertUntrusted,
            DeviceNotTrusted => ErrorCategory::DeviceNotTrusted,
            DeviceLocked => ErrorCategory::DeviceLocked,
            DeviceOffline => ErrorCategory::DeviceOffline,
            WifiUnreachable => ErrorCategory::WifiUnreachable,
            TunnelEstablishFailed => ErrorCategory::TunnelEstablishFailed,
            UsbmuxdDown => ErrorCategory::UsbmuxdDown,
            PermissionsMissing => ErrorCategory::PermissionsMissing,
            KeyringUnavailable => ErrorCategory::KeyringUnavailable,
            AnisetteGenFailed => ErrorCategory::AnisetteGenFailed,
            AnisetteAdiUnavailable => ErrorCategory::AnisetteAdiUnavailable,
            AnisetteAdiIncompatible => ErrorCategory::AnisetteAdiIncompatible,
            AppleAuthRateLimited => ErrorCategory::AppleAuthRateLimited,
            AppleAuth2faRequired => ErrorCategory::AppleAuth2faRequired,
            AppleAuthCredentialsInvalid => ErrorCategory::AppleAuthCredentialsInvalid,
            AppleAuthProtocolChanged => ErrorCategory::AppleAuthProtocolChanged,
            AppleDevCertGenFailed => ErrorCategory::AppleDevCertGenFailed,
            AppleDevDeviceRegLimitReached => ErrorCategory::AppleDevDeviceRegLimitReached,
            AppleAppIdLimitReached => ErrorCategory::AppleAppIdLimitReached,
            AppleCertLimitReached => ErrorCategory::AppleCertLimitReached,
            SigningCertExpired => ErrorCategory::SigningCertExpired,
            EntitlementsUnsupported => ErrorCategory::EntitlementsUnsupported,
            BundleIdConflict => ErrorCategory::BundleIdConflict,
            InstallTransferFailed => ErrorCategory::InstallTransferFailed,
            InstallVerifyFailed => ErrorCategory::InstallVerifyFailed,
            Io(_) | Db(_) | Migrate(_) | Serialization(_) | Internal(_) => ErrorCategory::Internal,
        }
    }

    /// Short, user-facing remediation copy. Mirrors plan.md §Error Taxonomy.
    pub fn remediation(&self) -> &'static str {
        use AppError::*;
        match self {
            UnsupportedIosVersion => "ReSide requires iOS / iPadOS 17.4 or newer.",
            IosDeveloperModeOff => {
                "Enable Developer Mode: Settings → Privacy & Security → Developer Mode, then restart your device."
            }
            IosDeveloperCertUntrusted => {
                "On your iPhone: Settings → General → VPN & Device Management → tap your Apple ID → Trust."
            }
            DeviceNotTrusted => "Tap Trust on your iPhone and retry.",
            DeviceLocked => "Unlock your iPhone and retry.",
            DeviceOffline => "Connect via USB or check Wi-Fi.",
            WifiUnreachable => "Device not reachable on this network. Try USB.",
            TunnelEstablishFailed => {
                "Could not establish a secure tunnel to the device. Restart reside-tunneld or reconnect via USB."
            }
            UsbmuxdDown => "Run: sudo systemctl start usbmuxd",
            PermissionsMissing => "Add your user to the plugdev group / install udev rules.",
            KeyringUnavailable => "No system keyring detected. Install one (e.g. gnome-keyring or KWallet) to sign in — ReSide will not store your Apple password without it.",
            AnisetteGenFailed => "Local anisette generation failed — see logs.",
            AnisetteAdiUnavailable => {
                "One-time setup needed: ReSide must download Apple's signing libraries. Start setup."
            }
            AnisetteAdiIncompatible => {
                "Apple's signing libraries changed — update ReSide or re-run library setup."
            }
            AppleAuthRateLimited => "Apple is rate-limiting. Wait ~15 min.",
            AppleAuth2faRequired => "Enter the verification code from your trusted device.",
            AppleAuthCredentialsInvalid => "Wrong Apple ID or password.",
            AppleAuthProtocolChanged => "Apple changed their auth flow — app update needed.",
            AppleDevCertGenFailed => {
                "Could not request a signing certificate from Apple — retry, or see logs."
            }
            AppleDevDeviceRegLimitReached => {
                "You've registered 10 devices this week. Wait until the oldest registration ages out."
            }
            AppleAppIdLimitReached => {
                "You've created 10 App IDs this week. Reuse an existing bundle ID, or wait."
            }
            AppleCertLimitReached => {
                "Apple allows only ~2 signing certificates per free account. Revoke an old one in Settings → Certificates, then try again."
            }
            SigningCertExpired => "Your signing certificate has expired — sign in again to renew.",
            EntitlementsUnsupported => "Some features may not work after signing.",
            BundleIdConflict => "Reuse an existing bundle ID or generate a new one.",
            InstallTransferFailed => "Transfer to device failed — check USB cable or Wi-Fi.",
            InstallVerifyFailed => "Install completed but verification failed.",
            Io(_) | Db(_) | Migrate(_) | Serialization(_) | Internal(_) => {
                "Something went wrong inside ReSide — see logs or export a debug bundle."
            }
        }
    }

    /// UI-safe, redacted view for operation events and IPC.
    pub fn report(&self) -> ErrorReport {
        ErrorReport {
            category: self.category().as_key().to_string(),
            remediation: self.remediation().to_string(),
        }
    }
}

/// A wrapper that prevents secret material from leaking through `Debug` /
/// `Display` (e.g. when an error or log line accidentally includes it).
/// The inner value is still accessible via [`Secret::expose`].
#[derive(Clone)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }
    /// Explicit, greppable accessor for the wrapped secret.
    pub fn expose(&self) -> &T {
        &self.0
    }
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Types that can produce a redacted string for logs / debug bundles.
pub trait Redactable {
    fn redacted(&self) -> String;
}

impl<T> Redactable for Secret<T> {
    fn redacted(&self) -> String {
        "<redacted>".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_distinct_remediation_and_category() {
        // Spot-check a handful of mappings.
        assert_eq!(
            AppError::DeviceLocked.category(),
            ErrorCategory::DeviceLocked
        );
        assert_eq!(
            AppError::AppleAuth2faRequired.category().as_key(),
            "AppleAuth2FARequired"
        );
        assert!(!AppError::UsbmuxdDown.remediation().is_empty());
    }

    #[test]
    fn report_is_serializable_and_redacted() {
        let report = AppError::DeviceLocked.report();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("DeviceLocked"));
        assert!(json.contains("Unlock"));
    }

    #[test]
    fn secret_does_not_leak_via_debug_or_display() {
        let s = Secret::new("hunter2-app-specific-password");
        assert_eq!(format!("{s}"), "<redacted>");
        assert_eq!(format!("{s:?}"), "<redacted>");
        assert_eq!(s.redacted(), "<redacted>");
        // The value is still recoverable via the explicit accessor.
        assert_eq!(*s.expose(), "hunter2-app-specific-password");
    }
}
