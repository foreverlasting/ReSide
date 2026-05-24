//! Read display metadata out of an `.ipa` without unpacking the whole archive.
//!
//! When ReSide hands an IPA to the forked signer it also records the app in
//! SQLite (see [`crate::installs`]) so the Dashboard and the refresh agent can
//! show and re-sign it later. That record needs the app's name, bundle id, and
//! version — all of which live in the main bundle's `Info.plist`. We read just
//! that one zip entry rather than extracting the (often hundreds of MB) IPA.

use crate::error::{AppError, Result};
use plist::Value;
use std::io::Read;
use std::path::Path;

/// The user-facing identity of an app, pulled from its `Info.plist`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpaMetadata {
    /// `CFBundleDisplayName`, falling back to `CFBundleName`, then the bundle id.
    pub display_name: String,
    /// `CFBundleIdentifier` — required; an IPA without one is malformed.
    pub bundle_id: String,
    /// `CFBundleShortVersionString` (the marketing version), falling back to
    /// `CFBundleVersion`. `None` if neither is present.
    pub version: Option<String>,
}

/// Read [`IpaMetadata`] from the main app bundle inside `ipa_path`.
///
/// An IPA can contain several `Info.plist` files (app extensions, embedded
/// frameworks). The main app's is the shallowest `Payload/<Name>.app/Info.plist`,
/// so we pick the matching entry with the fewest path segments.
pub fn read_ipa_metadata(ipa_path: &Path) -> Result<IpaMetadata> {
    let file = std::fs::File::open(ipa_path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Internal(format!("not a valid IPA archive: {e}")))?;

    let mut main: Option<(usize, String)> = None;
    for i in 0..zip.len() {
        let name = zip
            .by_index(i)
            .map_err(|e| AppError::Internal(format!("corrupt zip entry: {e}")))?
            .name()
            .to_string();
        if let Some(rest) = name.strip_prefix("Payload/") {
            // `<Name>.app/Info.plist` for the main bundle; deeper for nested ones.
            if rest.ends_with(".app/Info.plist") {
                let depth = rest.matches('/').count();
                if main.as_ref().map_or(true, |(d, _)| depth < *d) {
                    main = Some((depth, name));
                }
            }
        }
    }

    let entry_name = main
        .map(|(_, n)| n)
        .ok_or_else(|| AppError::Internal("IPA has no Payload/*.app/Info.plist".into()))?;

    let mut buf = Vec::new();
    zip.by_name(&entry_name)
        .map_err(|e| AppError::Internal(format!("cannot read Info.plist: {e}")))?
        .read_to_end(&mut buf)?;

    let value = Value::from_reader(std::io::Cursor::new(buf))
        .map_err(|e| AppError::Internal(format!("unreadable Info.plist: {e}")))?;
    let dict = value
        .as_dictionary()
        .ok_or_else(|| AppError::Internal("Info.plist is not a dictionary".into()))?;

    let str_key = |k: &str| dict.get(k).and_then(|v| v.as_string()).map(str::to_string);

    let bundle_id = str_key("CFBundleIdentifier")
        .ok_or_else(|| AppError::Internal("Info.plist missing CFBundleIdentifier".into()))?;
    let display_name = str_key("CFBundleDisplayName")
        .or_else(|| str_key("CFBundleName"))
        .unwrap_or_else(|| bundle_id.clone());
    let version = str_key("CFBundleShortVersionString").or_else(|| str_key("CFBundleVersion"));

    Ok(IpaMetadata {
        display_name,
        bundle_id,
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a minimal IPA: a main `Demo.app` plus a nested extension whose own
    /// Info.plist must NOT win the "main bundle" selection.
    fn synthetic_ipa(dir: &Path) -> std::path::PathBuf {
        let ipa = dir.join("Demo.ipa");
        let f = std::fs::File::create(&ipa).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();

        let main_plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.example.demo</string>
  <key>CFBundleDisplayName</key><string>Demo App</string>
  <key>CFBundleName</key><string>Demo</string>
  <key>CFBundleShortVersionString</key><string>2.3.0</string>
  <key>CFBundleVersion</key><string>230</string>
</dict></plist>"#;
        let ext_plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.example.demo.share</string>
  <key>CFBundleDisplayName</key><string>Share</string>
</dict></plist>"#;

        // Nested extension entry first, to prove ordering doesn't matter.
        zw.start_file("Payload/Demo.app/PlugIns/Share.appex/Info.plist", opts)
            .unwrap();
        zw.write_all(ext_plist.as_bytes()).unwrap();
        zw.start_file("Payload/Demo.app/Info.plist", opts).unwrap();
        zw.write_all(main_plist.as_bytes()).unwrap();
        zw.finish().unwrap();
        ipa
    }

    #[test]
    fn reads_main_bundle_metadata_not_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let ipa = synthetic_ipa(tmp.path());
        let meta = read_ipa_metadata(&ipa).unwrap();
        assert_eq!(meta.bundle_id, "com.example.demo");
        assert_eq!(meta.display_name, "Demo App");
        assert_eq!(meta.version.as_deref(), Some("2.3.0"));
    }

    #[test]
    fn falls_back_to_bundle_name_then_version() {
        let tmp = tempfile::tempdir().unwrap();
        let ipa = tmp.path().join("min.ipa");
        let f = std::fs::File::create(&ipa).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file(
            "Payload/Min.app/Info.plist",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zw.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.example.min</string>
  <key>CFBundleName</key><string>Min</string>
  <key>CFBundleVersion</key><string>7</string>
</dict></plist>"#,
        )
        .unwrap();
        zw.finish().unwrap();

        let meta = read_ipa_metadata(&ipa).unwrap();
        assert_eq!(meta.display_name, "Min"); // no DisplayName -> Name
        assert_eq!(meta.version.as_deref(), Some("7")); // no Short -> Version
    }

    #[test]
    fn rejects_archive_without_app_plist() {
        let tmp = tempfile::tempdir().unwrap();
        let ipa = tmp.path().join("empty.ipa");
        let f = std::fs::File::create(&ipa).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file("README.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zw.write_all(b"not an app").unwrap();
        zw.finish().unwrap();

        assert!(matches!(
            read_ipa_metadata(&ipa),
            Err(AppError::Internal(_))
        ));
    }
}
