//! Bundle-ID rewriter: prefix/replace per signing method, including nested
//! extensions/frameworks. Default policy reuses existing IDs to stay under the
//! weekly App-ID quota. Phase 2.
//!
//! iOS app bundles nest extensions (`.appex`) and frameworks whose own
//! `CFBundleIdentifier`s are usually children of the main app's ID
//! (`com.example.app` → `com.example.app.MyExtension`). When the main ID
//! changes, those child IDs must move with it or the extension won't be
//! associated with the re-signed app. Frameworks with unrelated IDs are left
//! alone (they are not registered as App IDs).

/// How the main app's bundle identifier should be transformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleIdStrategy {
    /// Reuse the original identifier verbatim. Default: keeps us under Apple's
    /// 10-App-IDs-per-week quota on the free path.
    Keep,
    /// Replace the main identifier outright (e.g. with a profile's App ID).
    Replace(String),
    /// Prepend a reverse-DNS prefix segment, e.g. `me.reside` →
    /// `me.reside.com.example.app`.
    Prefix(String),
}

/// Compute the new main-app identifier under `strategy`.
pub fn rewritten_main(original: &str, strategy: &BundleIdStrategy) -> String {
    match strategy {
        BundleIdStrategy::Keep => original.to_string(),
        BundleIdStrategy::Replace(id) => id.clone(),
        BundleIdStrategy::Prefix(prefix) => {
            let prefix = prefix.trim_matches('.');
            if prefix.is_empty() {
                original.to_string()
            } else {
                format!("{prefix}.{original}")
            }
        }
    }
}

/// Rewrite a nested bundle's identifier to follow the main app's move.
///
/// If `nested_original` is the main ID or a dotted child of it, its prefix is
/// retargeted from `original_main` to `new_main` (the suffix is preserved).
/// Otherwise it is returned unchanged.
pub fn rewrite_nested(nested_original: &str, original_main: &str, new_main: &str) -> String {
    if nested_original == original_main {
        return new_main.to_string();
    }
    let child_prefix = format!("{original_main}.");
    if let Some(suffix) = nested_original.strip_prefix(&child_prefix) {
        format!("{new_main}.{suffix}")
    } else {
        nested_original.to_string()
    }
}

/// Light validity check for a bundle identifier: non-empty, dot-separated
/// segments of alphanumerics and hyphens (Apple also tolerates `*` in wildcard
/// App IDs, but a concrete bundle ID should not contain one).
pub fn is_valid_bundle_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    id.split('.').all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_is_identity() {
        assert_eq!(
            rewritten_main("com.example.app", &BundleIdStrategy::Keep),
            "com.example.app"
        );
    }

    #[test]
    fn replace_sets_outright() {
        assert_eq!(
            rewritten_main(
                "com.example.app",
                &BundleIdStrategy::Replace("me.reside.hello".into())
            ),
            "me.reside.hello"
        );
    }

    #[test]
    fn prefix_prepends_and_trims_dots() {
        assert_eq!(
            rewritten_main(
                "com.example.app",
                &BundleIdStrategy::Prefix("me.reside".into())
            ),
            "me.reside.com.example.app"
        );
        assert_eq!(
            rewritten_main(
                "com.example.app",
                &BundleIdStrategy::Prefix(".me.reside.".into())
            ),
            "me.reside.com.example.app"
        );
        // Empty prefix is a no-op rather than producing a leading dot.
        assert_eq!(
            rewritten_main("com.example.app", &BundleIdStrategy::Prefix("".into())),
            "com.example.app"
        );
    }

    #[test]
    fn nested_child_follows_the_main_move() {
        assert_eq!(
            rewrite_nested(
                "com.example.app.ShareExtension",
                "com.example.app",
                "me.reside.com.example.app"
            ),
            "me.reside.com.example.app.ShareExtension"
        );
    }

    #[test]
    fn nested_equal_to_main_is_retargeted() {
        assert_eq!(
            rewrite_nested("com.example.app", "com.example.app", "new.id"),
            "new.id"
        );
    }

    #[test]
    fn unrelated_nested_id_is_untouched() {
        // A vendored framework with its own reverse-DNS id is not a child.
        assert_eq!(
            rewrite_nested("org.cocoapods.Alamofire", "com.example.app", "new.id"),
            "org.cocoapods.Alamofire"
        );
        // A partial-but-not-dotted match must not be treated as a child.
        assert_eq!(
            rewrite_nested("com.example.apptastic", "com.example.app", "new.id"),
            "com.example.apptastic"
        );
    }

    #[test]
    fn validity_check() {
        assert!(is_valid_bundle_id("com.example.app"));
        assert!(is_valid_bundle_id("me.reside.com.example.app-pro"));
        assert!(!is_valid_bundle_id(""));
        assert!(!is_valid_bundle_id("com..app"));
        assert!(!is_valid_bundle_id("com.example.app*"));
        assert!(!is_valid_bundle_id("com.exa mple.app"));
    }
}
