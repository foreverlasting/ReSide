//! Paid developer cert signing provider: import `.p12` + `.mobileprovision`
//! from disk. No Apple auth, no ADI libs. Phase 2 (built first — easiest to
//! test).
//!
//! ⚠️ **PARKED** — part of the abandoned native-signing path. The live app
//! drives the forked Sideloader CLI; this provider is unused. Kept for
//! reference. See [`super`] (signing/mod.rs) and [`crate::signer`].
//!
//! A `.p12`/PFX holds the developer certificate and its private key. A
//! `.mobileprovision` is a CMS (PKCS#7 SignedData) envelope wrapping an XML
//! plist that lists the authorized entitlements, team, and devices. This
//! provider loads both off disk and exposes them to [`super::ipa_pipeline`]
//! via the [`SigningProvider`] trait.

use std::io::Cursor;
use std::path::Path;

use apple_codesign::cryptography::{parse_pfx_data, InMemoryPrivateKey, PrivateKey};
use plist::{Dictionary, Value};
use x509_certificate::{CapturedX509Certificate, KeyInfoSigner};

use super::{ProvisioningProfile, SigningMethod, SigningProvider};
use crate::error::{AppError, Result};

/// A signing identity imported from a paid Apple Developer certificate plus a
/// provisioning profile.
pub struct PaidCertProvider {
    certificate: CapturedX509Certificate,
    key: InMemoryPrivateKey,
    profile: ProvisioningProfile,
    team_id: Option<String>,
}

impl PaidCertProvider {
    /// Import a `.p12` (certificate + private key, unlocked with `p12_password`)
    /// and a `.mobileprovision` from disk.
    pub fn import(
        p12_path: &Path,
        p12_password: &str,
        mobileprovision_path: &Path,
    ) -> Result<Self> {
        let p12_data = std::fs::read(p12_path).map_err(AppError::Io)?;
        let (certificate, key) = parse_pfx_data(&p12_data, p12_password).map_err(|e| {
            // Includes the wrong-password case (PfxBadPassword). No dedicated
            // taxonomy entry yet; surfaced as an internal error.
            AppError::Internal(format!("could not open .p12: {e}"))
        })?;

        let raw = std::fs::read(mobileprovision_path).map_err(AppError::Io)?;
        let profile = parse_mobileprovision(raw)?;

        // Prefer the profile's team; fall back to the entitlements team key.
        let team_id = profile.team_id.clone();

        Ok(Self {
            certificate,
            key,
            profile,
            team_id,
        })
    }
}

impl SigningProvider for PaidCertProvider {
    fn method(&self) -> SigningMethod {
        SigningMethod::PaidCert
    }
    fn certificate(&self) -> &CapturedX509Certificate {
        &self.certificate
    }
    fn signing_key(&self) -> &dyn KeyInfoSigner {
        self.key.as_key_info_signer()
    }
    fn provisioning_profile(&self) -> Option<&ProvisioningProfile> {
        Some(&self.profile)
    }
    fn team_id(&self) -> Option<&str> {
        self.team_id.as_deref()
    }
}

/// Parse a `.mobileprovision` into a [`ProvisioningProfile`].
///
/// The file is a CMS SignedData blob; rather than do full CMS parsing we
/// extract the embedded XML plist (the well-known, robust approach — a
/// mobileprovision carries exactly one `<plist>…</plist>`).
fn parse_mobileprovision(raw: Vec<u8>) -> Result<ProvisioningProfile> {
    let plist_bytes = extract_embedded_plist(&raw)?;
    let value = Value::from_reader_xml(Cursor::new(plist_bytes))
        .map_err(|e| AppError::Internal(format!("unreadable provisioning profile: {e}")))?;
    let dict = value
        .as_dictionary()
        .ok_or_else(|| AppError::Internal("provisioning profile is not a dictionary".into()))?;

    let entitlements = dict
        .get("Entitlements")
        .and_then(|v| v.as_dictionary())
        .cloned()
        .unwrap_or_else(Dictionary::new);

    let team_id = dict
        .get("TeamIdentifier")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_string())
        .map(str::to_string)
        .or_else(|| {
            entitlements
                .get("com.apple.developer.team-identifier")
                .and_then(|v| v.as_string())
                .map(str::to_string)
        });

    let application_identifier = entitlements
        .get("application-identifier")
        .and_then(|v| v.as_string())
        .map(str::to_string);

    Ok(ProvisioningProfile {
        raw,
        entitlements,
        team_id,
        application_identifier,
    })
}

/// Find the `<?xml … </plist>` span embedded in the CMS envelope.
fn extract_embedded_plist(data: &[u8]) -> Result<&[u8]> {
    const START: &[u8] = b"<?xml";
    const END: &[u8] = b"</plist>";
    let start = find_subsequence(data, START)
        .ok_or_else(|| AppError::Internal("provisioning profile has no plist".into()))?;
    let end_rel = find_subsequence(&data[start..], END)
        .ok_or_else(|| AppError::Internal("provisioning profile plist is truncated".into()))?;
    let end = start + end_rel + END.len();
    Ok(&data[start..end])
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap_cms_like(plist_xml: &str) -> Vec<u8> {
        // Simulate the CMS envelope: arbitrary binary noise around the plist.
        let mut v = vec![0x30, 0x82, 0x01, 0x00, 0xde, 0xad, 0xbe, 0xef];
        v.extend_from_slice(plist_xml.as_bytes());
        v.extend_from_slice(&[0x00, 0x01, 0x02, 0x03]);
        v
    }

    #[test]
    fn extracts_and_parses_embedded_plist() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>TeamIdentifier</key>
  <array><string>ABCDE12345</string></array>
  <key>Entitlements</key>
  <dict>
    <key>application-identifier</key>
    <string>ABCDE12345.com.example.app</string>
    <key>get-task-allow</key>
    <true/>
  </dict>
</dict>
</plist>"#;
        let raw = wrap_cms_like(xml);
        let profile = parse_mobileprovision(raw).expect("parse");
        assert_eq!(profile.team_id.as_deref(), Some("ABCDE12345"));
        assert_eq!(
            profile.application_identifier.as_deref(),
            Some("ABCDE12345.com.example.app")
        );
        assert!(profile.entitlements.contains_key("get-task-allow"));
    }

    #[test]
    fn missing_plist_is_an_error() {
        let raw = vec![0xde, 0xad, 0xbe, 0xef];
        assert!(parse_mobileprovision(raw).is_err());
    }

    #[test]
    fn find_subsequence_basics() {
        assert_eq!(find_subsequence(b"hello world", b"world"), Some(6));
        assert_eq!(find_subsequence(b"hello", b"xyz"), None);
        assert_eq!(find_subsequence(b"abc", b""), None);
    }
}
