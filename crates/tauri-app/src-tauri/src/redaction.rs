//! Redaction helpers for payloads that cross the Tauri boundary or land in
//! logs / debug bundles. Apple ID strings, raw UDIDs, and secret material must
//! never be serialized verbatim (see plan.md §Secrets & Redaction).

/// Redact a device UDID, keeping a short suffix for human correlation.
/// `00008110-001A4D2E1E78801E` → `udid:…801E`.
pub fn redact_udid(udid: &str) -> String {
    let tail: String = udid
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("udid:…{tail}")
}

/// Hash-shaped redaction for an Apple ID (never log the address itself).
pub fn redact_apple_id(_apple_id: &str) -> &'static str {
    "<apple-id>"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udid_is_redacted_to_suffix() {
        assert_eq!(redact_udid("00008110-001A4D2E1E78801E"), "udid:…801E");
    }

    #[test]
    fn apple_id_never_echoed() {
        assert_eq!(redact_apple_id("maya@example.com"), "<apple-id>");
    }
}
