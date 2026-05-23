//! Signing providers + the IPA signing pipeline.
//!
//! Two `SigningProvider` implementations feed one `ipa_pipeline`:
//! `free_apple_id` (omnisette + icloud-auth + apple-dev-apis, with ADI FFI) and
//! `paid_cert` (import p12 + .mobileprovision). The trait is the project's
//! insurance policy against upstream churn — do not collapse it. Phase 2.

pub mod adi;
pub mod bundle_id;
pub mod entitlements;
pub mod free_apple_id;
pub mod ipa_pipeline;
pub mod paid_cert;
pub mod quota;

use x509_certificate::{CapturedX509Certificate, KeyInfoSigner};

/// Which signing method (credential source) is in play. Drives bundle-ID reuse
/// and entitlement policy in the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningMethod {
    /// Imported paid Apple Developer certificate (`.p12` + `.mobileprovision`).
    /// Entitlements are preserved; no weekly App-ID quota applies.
    PaidCert,
    /// Free Apple ID. Premium entitlements are stripped; the provisioning
    /// profile lives 7 days; Apple weekly quotas apply. Phase 2 (later).
    FreeAppleId,
}

/// A parsed Apple provisioning profile (`.mobileprovision`).
///
/// The on-disk file is a CMS (PKCS#7 SignedData) envelope wrapping an XML
/// plist. We keep the original bytes (to embed into the bundle as
/// `embedded.mobileprovision`) alongside the fields the pipeline needs.
#[derive(Clone)]
pub struct ProvisioningProfile {
    /// Original `.mobileprovision` bytes, embedded verbatim into the bundle.
    pub raw: Vec<u8>,
    /// The `Entitlements` dictionary from the profile.
    pub entitlements: plist::Dictionary,
    /// `TeamIdentifier` (first entry), if present.
    pub team_id: Option<String>,
    /// `application-identifier` entitlement, e.g. `ABCDE12345.com.example.app`.
    pub application_identifier: Option<String>,
}

impl std::fmt::Debug for ProvisioningProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvisioningProfile")
            .field("raw_len", &self.raw.len())
            .field("team_id", &self.team_id)
            .field("application_identifier", &self.application_identifier)
            .finish()
    }
}

/// A replaceable signing adapter. Both the paid-cert path and the (future)
/// free-Apple-ID path implement this so [`ipa_pipeline`] never has to know how
/// the credentials were obtained. Do not collapse this trait — it is the
/// project's insurance policy against upstream Apple-auth churn
/// (see plan.md §Known Foot-Guns).
pub trait SigningProvider {
    fn method(&self) -> SigningMethod;

    /// The code-signing certificate (public half).
    fn certificate(&self) -> &CapturedX509Certificate;

    /// The private key, as a signer. Borrowed for the lifetime of the provider.
    fn signing_key(&self) -> &dyn KeyInfoSigner;

    /// The provisioning profile to embed, if the method uses one. The
    /// self-signed development path has none.
    fn provisioning_profile(&self) -> Option<&ProvisioningProfile>;

    /// Apple team identifier, if known (from the cert or the profile).
    fn team_id(&self) -> Option<&str>;
}

/// Test-only self-signed development provider. Useful for exercising the full
/// unzip → patch → sign → repack → verify pipeline without a real Apple cert or
/// device. Self-signed certs are NOT accepted by real iOS devices — this is for
/// mechanical CI validation only.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use apple_codesign::{create_self_signed_code_signing_certificate, CertificateProfile};
    use x509_certificate::{EcdsaCurve, InMemorySigningKeyPair, KeyAlgorithm};

    pub(crate) struct SelfSignedProvider {
        cert: CapturedX509Certificate,
        key: InMemorySigningKeyPair,
        team_id: String,
    }

    impl SelfSignedProvider {
        pub(crate) fn generate() -> Self {
            let team_id = "TESTTEAM00";
            let (cert, key) = create_self_signed_code_signing_certificate(
                KeyAlgorithm::Ecdsa(EcdsaCurve::Secp256r1),
                CertificateProfile::AppleDevelopment,
                team_id,
                "ReSide Test",
                "US",
                chrono::Duration::days(365),
            )
            .expect("self-signed cert generation");
            Self {
                cert,
                key,
                team_id: team_id.to_string(),
            }
        }
    }

    impl SigningProvider for SelfSignedProvider {
        fn method(&self) -> SigningMethod {
            SigningMethod::PaidCert
        }
        fn certificate(&self) -> &CapturedX509Certificate {
            &self.cert
        }
        fn signing_key(&self) -> &dyn KeyInfoSigner {
            &self.key
        }
        fn provisioning_profile(&self) -> Option<&ProvisioningProfile> {
            None
        }
        fn team_id(&self) -> Option<&str> {
            Some(&self.team_id)
        }
    }
}
