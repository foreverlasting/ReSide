//! Unzip → patch → sign → repack pipeline. `apple-codesign` signs `.app`
//! bundles; IPA wrapping (zip, alignment, symlink preservation) is ours.
//! Phase 2.
//!
//! ⚠️ **PARKED** — part of the abandoned native-signing path. The live app
//! drives the forked Sideloader CLI for signing; nothing in `reside-app`
//! calls this pipeline. Kept for reference / the `native-signing-path` branch.
//! See [`super`] (signing/mod.rs) and [`crate::signer`] for the live path.
//!
//! Flow ([`sign_ipa`]):
//! 1. Extract the `.ipa` (a zip) to a scratch dir, preserving symlinks + modes.
//! 2. Locate `Payload/<Name>.app` and read its `Info.plist`.
//! 3. Optionally rewrite bundle identifiers ([`super::bundle_id`]).
//! 4. Resolve + filter entitlements ([`super::entitlements`]); embed the
//!    provisioning profile if the provider has one.
//! 5. Sign the bundle with `apple-codesign`'s `BundleSigner`, which signs
//!    nested frameworks/extensions bottom-up before the main app.
//! 6. Repack the signed bundle into a new `.ipa`.
//! 7. Verify the signed main executable carries a code signature.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use apple_codesign::{BundleSigner, MachOBinary, SettingsScope, SigningSettings};
use plist::{Dictionary, Value};
use tracing::{debug, info, warn};

use super::bundle_id::{self, BundleIdStrategy};
use super::{entitlements, SigningProvider};
use crate::error::{AppError, Result};

/// Options controlling a signing run.
#[derive(Debug, Clone)]
pub struct SignOptions {
    /// How to transform the main app's bundle identifier. Default [`BundleIdStrategy::Keep`].
    pub bundle_id_strategy: BundleIdStrategy,
}

impl Default for SignOptions {
    fn default() -> Self {
        Self {
            bundle_id_strategy: BundleIdStrategy::Keep,
        }
    }
}

/// Summary of a completed signing run.
#[derive(Debug, Clone)]
pub struct SignReport {
    pub original_bundle_id: String,
    pub bundle_id: String,
    /// The `.app` directory name inside `Payload/`, e.g. `Apollo.app`.
    pub app_bundle_name: String,
    pub team_id: Option<String>,
    /// Entitlements removed by the per-method filter (free path only).
    pub stripped_entitlements: Vec<String>,
    pub output_path: PathBuf,
}

/// Sign `input_ipa` with `provider`, writing a new IPA to `output_ipa`.
pub fn sign_ipa(
    provider: &dyn SigningProvider,
    input_ipa: &Path,
    output_ipa: &Path,
    opts: &SignOptions,
) -> Result<SignReport> {
    let work = tempfile::Builder::new()
        .prefix("reside-sign-")
        .tempdir()
        .map_err(AppError::Io)?;
    let extract_dir = work.path().join("extracted");

    info!(ipa = %input_ipa.display(), "extracting IPA");
    extract_zip(input_ipa, &extract_dir)?;

    let app_dir = find_app_dir(&extract_dir)?;
    let app_bundle_name = app_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::Internal("app bundle has no name".into()))?
        .to_string();

    let info = read_info_plist(&app_dir)?;
    let original_bundle_id = string_key(&info, "CFBundleIdentifier")
        .ok_or_else(|| AppError::Internal("Info.plist missing CFBundleIdentifier".into()))?;
    let main_executable = string_key(&info, "CFBundleExecutable")
        .ok_or_else(|| AppError::Internal("Info.plist missing CFBundleExecutable".into()))?;

    // 3. Bundle-ID rewrite (skipped entirely for the default Keep policy so we
    //    never touch a well-formed Info.plist unnecessarily).
    let new_bundle_id = bundle_id::rewritten_main(&original_bundle_id, &opts.bundle_id_strategy);
    if new_bundle_id != original_bundle_id {
        if !bundle_id::is_valid_bundle_id(&new_bundle_id) {
            return Err(AppError::BundleIdConflict);
        }
        info!(from = %original_bundle_id, to = %new_bundle_id, "rewriting bundle identifiers");
        rewrite_all_bundle_ids(&app_dir, &original_bundle_id, &new_bundle_id)?;
    }

    // 4. Entitlements + provisioning profile.
    let profile = provider.provisioning_profile();
    let base_entitlements = match profile {
        Some(p) => p.entitlements.clone(),
        None => default_development_entitlements(&new_bundle_id, provider.team_id()),
    };
    let filtered = entitlements::filter(&base_entitlements, provider.method());
    if !filtered.stripped.is_empty() {
        warn!(stripped = ?filtered.stripped, "stripped entitlements unsupported by this signing method");
    }
    let entitlements_xml = dictionary_to_xml(&filtered.kept)?;

    if let Some(p) = profile {
        embed_provisioning_profile(&app_dir, &p.raw)?;
    }

    // 5. Sign. `BundleSigner` walks nested bundles and signs leaf-first.
    let signed_root = work.path().join("signed");
    fs::create_dir_all(&signed_root).map_err(AppError::Io)?;
    let signed_app = signed_root.join(&app_bundle_name);

    let cert = provider.certificate().clone();
    let key = provider.signing_key();
    let mut settings = SigningSettings::default();
    settings.set_signing_key(key, cert);
    settings
        .set_entitlements_xml(SettingsScope::Main, &entitlements_xml)
        .map_err(|e| signing_error("set entitlements", e))?;
    if let Some(team) = provider.team_id() {
        settings.set_team_id(team);
    }

    let mut signer =
        BundleSigner::new_from_path(&app_dir).map_err(|e| signing_error("open bundle", e))?;
    signer
        .collect_nested_bundles()
        .map_err(|e| signing_error("collect nested bundles", e))?;
    signer
        .write_signed_bundle(&signed_app, &settings)
        .map_err(|e| signing_error("write signed bundle", e))?;

    // 6. Repack.
    info!(out = %output_ipa.display(), "repacking signed IPA");
    repack_ipa(&signed_app, &app_bundle_name, output_ipa)?;

    // 7. Verify the main executable is signed.
    verify_macho_signed(&signed_app.join(&main_executable))?;

    Ok(SignReport {
        original_bundle_id,
        bundle_id: new_bundle_id,
        app_bundle_name,
        team_id: provider.team_id().map(str::to_string),
        stripped_entitlements: filtered.stripped,
        output_path: output_ipa.to_path_buf(),
    })
}

fn signing_error(ctx: &str, e: apple_codesign::AppleCodesignError) -> AppError {
    // apple-codesign error text can be verbose; log it, surface a redacted internal error.
    debug!(context = ctx, error = %e, "apple-codesign signing error");
    AppError::Internal(format!("signing failed: {ctx}"))
}

fn string_key(dict: &Dictionary, key: &str) -> Option<String> {
    dict.get(key)
        .and_then(|v| v.as_string())
        .map(str::to_string)
}

/// Extract a zip archive to `dest`, preserving symlinks and unix permissions,
/// and guarding against zip-slip path traversal via `enclosed_name`.
fn extract_zip(archive_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive_path).map_err(AppError::Io)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| AppError::Internal(format!("not a valid IPA archive: {e}")))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::Internal(format!("corrupt zip entry: {e}")))?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(AppError::Internal(
                "IPA contains an unsafe (path-traversing) entry".into(),
            ));
        };
        let out_path = dest.join(&rel);

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(AppError::Io)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(AppError::Io)?;
        }

        if entry.is_symlink() {
            let mut target = String::new();
            entry.read_to_string(&mut target).map_err(AppError::Io)?;
            // `enclosed_name` guards the entry's *own* path, but the symlink
            // target is arbitrary: an absolute or `..`-laden target would let a
            // later regular-file entry be written *through* this link, outside
            // `dest` (symlink zip-slip). Reject any target that doesn't stay
            // within the extraction root.
            if !symlink_target_stays_within(dest, &out_path, &target) {
                return Err(AppError::Internal(
                    "IPA contains a symlink escaping the archive root".into(),
                ));
            }
            // Replace an existing path (re-extraction) before symlinking.
            let _ = fs::remove_file(&out_path);
            std::os::unix::fs::symlink(&target, &out_path).map_err(AppError::Io)?;
            continue;
        }

        let mut out = File::create(&out_path).map_err(AppError::Io)?;
        std::io::copy(&mut entry, &mut out).map_err(AppError::Io)?;
        if let Some(mode) = entry.unix_mode() {
            fs::set_permissions(&out_path, fs::Permissions::from_mode(mode))
                .map_err(AppError::Io)?;
        }
    }
    Ok(())
}

/// Whether a symlink at `link_path` pointing at `target` resolves to a location
/// inside `root`. Absolute targets are rejected outright; relative ones are
/// resolved against the link's parent and checked component-by-component so a
/// `..` sequence can never climb above `root`. Purely lexical (no filesystem
/// access), which is what we want: the target may not exist yet at extraction.
fn symlink_target_stays_within(root: &Path, link_path: &Path, target: &str) -> bool {
    use std::path::Component;

    let target = Path::new(target);
    if target.is_absolute() {
        return false;
    }
    let Some(link_parent) = link_path.parent() else {
        return false;
    };

    // Normalize `link_parent` (already under `root`) joined with `target`,
    // collapsing `.`/`..` lexically. Any `..` that would pop past `root` fails.
    let root_depth = root.components().count();
    let mut depth = link_parent.components().count();
    for comp in target.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
                if depth < root_depth {
                    return false;
                }
            }
            Component::Normal(_) => depth += 1,
            // A root or prefix component inside a relative path shouldn't occur,
            // but treat it as an escape rather than trusting it.
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// Find the single `Payload/<Name>.app` directory.
fn find_app_dir(extract_dir: &Path) -> Result<PathBuf> {
    let payload = extract_dir.join("Payload");
    if !payload.is_dir() {
        return Err(AppError::Internal("IPA has no Payload directory".into()));
    }
    for entry in fs::read_dir(&payload).map_err(AppError::Io)? {
        let path = entry.map_err(AppError::Io)?.path();
        if path.is_dir() && path.extension().and_then(|e| e.to_str()) == Some("app") {
            return Ok(path);
        }
    }
    Err(AppError::Internal(
        "IPA Payload contains no .app bundle".into(),
    ))
}

fn read_info_plist(app_dir: &Path) -> Result<Dictionary> {
    let path = app_dir.join("Info.plist");
    let value = Value::from_file(&path)
        .map_err(|e| AppError::Internal(format!("unreadable Info.plist: {e}")))?;
    value
        .into_dictionary()
        .ok_or_else(|| AppError::Internal("Info.plist is not a dictionary".into()))
}

/// Rewrite `CFBundleIdentifier` in the main app and every nested bundle whose
/// id is a child of the original main id. Identifiers that belong to vendored
/// frameworks (unrelated reverse-DNS) are left untouched by [`bundle_id::rewrite_nested`].
fn rewrite_all_bundle_ids(app_dir: &Path, original_main: &str, new_main: &str) -> Result<()> {
    let mut info_plists = Vec::new();
    collect_info_plists(app_dir, &mut info_plists)?;
    for plist_path in info_plists {
        let mut value = Value::from_file(&plist_path)
            .map_err(|e| AppError::Internal(format!("unreadable nested Info.plist: {e}")))?;
        let Some(dict) = value.as_dictionary_mut() else {
            continue;
        };
        let Some(current) = dict.get("CFBundleIdentifier").and_then(|v| v.as_string()) else {
            continue;
        };
        let rewritten = bundle_id::rewrite_nested(current, original_main, new_main);
        if rewritten != current {
            dict.insert("CFBundleIdentifier".into(), Value::from(rewritten));
            write_info_plist(&plist_path, &value)?;
        }
    }
    Ok(())
}

/// Collect `Info.plist` paths for the main bundle and nested bundles, without
/// following symlinks (avoids loops via framework symlinks on macOS-style bundles).
fn collect_info_plists(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let info = dir.join("Info.plist");
    if info.is_file() {
        out.push(info);
    }
    for entry in fs::read_dir(dir).map_err(AppError::Io)? {
        let path = entry.map_err(AppError::Io)?.path();
        let meta = fs::symlink_metadata(&path).map_err(AppError::Io)?;
        if meta.is_dir() {
            collect_info_plists(&path, out)?;
        }
    }
    Ok(())
}

/// Write an Info.plist back, preserving its on-disk format (binary vs XML).
fn write_info_plist(path: &Path, value: &Value) -> Result<()> {
    if is_binary_plist(path)? {
        value
            .to_file_binary(path)
            .map_err(|e| AppError::Internal(format!("failed writing Info.plist: {e}")))
    } else {
        value
            .to_file_xml(path)
            .map_err(|e| AppError::Internal(format!("failed writing Info.plist: {e}")))
    }
}

fn is_binary_plist(path: &Path) -> Result<bool> {
    let mut magic = [0u8; 8];
    let mut f = File::open(path).map_err(AppError::Io)?;
    let n = f.read(&mut magic).map_err(AppError::Io)?;
    Ok(n >= 8 && &magic == b"bplist00")
}

/// Minimal entitlements for the self-signed development path (no profile).
fn default_development_entitlements(bundle_id: &str, team_id: Option<&str>) -> Dictionary {
    let mut d = Dictionary::new();
    d.insert("get-task-allow".into(), Value::Boolean(true));
    if let Some(team) = team_id {
        d.insert(
            "application-identifier".into(),
            Value::from(format!("{team}.{bundle_id}")),
        );
        d.insert(
            "com.apple.developer.team-identifier".into(),
            Value::from(team),
        );
    }
    d
}

fn dictionary_to_xml(dict: &Dictionary) -> Result<String> {
    let value = Value::Dictionary(dict.clone());
    let mut buf = Vec::new();
    value
        .to_writer_xml(&mut buf)
        .map_err(|e| AppError::Internal(format!("entitlements serialization failed: {e}")))?;
    String::from_utf8(buf).map_err(|e| AppError::Internal(format!("entitlements not UTF-8: {e}")))
}

/// Embed the provisioning profile into the main bundle and each app extension.
fn embed_provisioning_profile(app_dir: &Path, raw: &[u8]) -> Result<()> {
    fs::write(app_dir.join("embedded.mobileprovision"), raw).map_err(AppError::Io)?;
    let plugins = app_dir.join("PlugIns");
    if plugins.is_dir() {
        for entry in fs::read_dir(&plugins).map_err(AppError::Io)? {
            let path = entry.map_err(AppError::Io)?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("appex") {
                fs::write(path.join("embedded.mobileprovision"), raw).map_err(AppError::Io)?;
            }
        }
    }
    Ok(())
}

/// Repack the signed `.app` into `output` as `Payload/<app_bundle_name>/...`.
fn repack_ipa(signed_app: &Path, app_bundle_name: &str, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(AppError::Io)?;
    }
    let file = File::create(output).map_err(AppError::Io)?;
    let mut zw = zip::ZipWriter::new(BufWriter::new(file));
    let prefix = format!("Payload/{app_bundle_name}");

    let mut entries = Vec::new();
    collect_bundle_entries(signed_app, signed_app, &mut entries)?;
    // Stable, deterministic order (parents sort before children).
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    for (rel, kind) in entries {
        let name = format!("{prefix}/{}", rel.to_string_lossy());
        match kind {
            EntryKind::Dir => {
                zw.add_directory(format!("{name}/"), zip::write::SimpleFileOptions::default())
                    .map_err(zip_error)?;
            }
            EntryKind::Symlink(target) => {
                zw.add_symlink(name, target, zip::write::SimpleFileOptions::default())
                    .map_err(zip_error)?;
            }
            EntryKind::File(abs, mode) => {
                let opts = zip::write::SimpleFileOptions::default().unix_permissions(mode);
                zw.start_file(name, opts).map_err(zip_error)?;
                let mut f = File::open(&abs).map_err(AppError::Io)?;
                std::io::copy(&mut f, &mut zw).map_err(AppError::Io)?;
            }
        }
    }
    zw.finish().map_err(zip_error)?;
    Ok(())
}

enum EntryKind {
    Dir,
    File(PathBuf, u32),
    Symlink(String),
}

fn collect_bundle_entries(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(PathBuf, EntryKind)>,
) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(AppError::Io)? {
        let path = entry.map_err(AppError::Io)?.path();
        let rel = path
            .strip_prefix(root)
            .map_err(|_| AppError::Internal("path escaped bundle root".into()))?
            .to_path_buf();
        let meta = fs::symlink_metadata(&path).map_err(AppError::Io)?;
        let ft = meta.file_type();
        if ft.is_symlink() {
            let target = fs::read_link(&path).map_err(AppError::Io)?;
            out.push((
                rel,
                EntryKind::Symlink(target.to_string_lossy().into_owned()),
            ));
        } else if ft.is_dir() {
            out.push((rel, EntryKind::Dir));
            collect_bundle_entries(root, &path, out)?;
        } else {
            out.push((rel, EntryKind::File(path, meta.permissions().mode())));
        }
    }
    Ok(())
}

fn zip_error(e: zip::result::ZipError) -> AppError {
    AppError::Internal(format!("IPA repack failed: {e}"))
}

/// Confirm a (thin or fat) Mach-O carries an embedded code signature.
fn verify_macho_signed(exe_path: &Path) -> Result<()> {
    let data = fs::read(exe_path).map_err(AppError::Io)?;
    let slices = macho_slices(&data)?;
    if slices.is_empty() {
        return Err(AppError::InstallVerifyFailed);
    }
    for slice in slices {
        let macho = MachOBinary::parse(slice)
            .map_err(|e| AppError::Internal(format!("verify: unparseable Mach-O: {e}")))?;
        let sig = macho
            .code_signature()
            .map_err(|e| AppError::Internal(format!("verify: signature read failed: {e}")))?;
        if sig.is_none() {
            warn!(exe = %exe_path.display(), "signed binary is missing a code signature");
            return Err(AppError::InstallVerifyFailed);
        }
    }
    Ok(())
}

/// Return the Mach-O slices in `data`: one for a thin binary, N for a fat
/// (universal) binary. Fat header fields are big-endian per the Mach-O ABI.
fn macho_slices(data: &[u8]) -> Result<Vec<&[u8]>> {
    if data.len() < 8 {
        return Ok(Vec::new());
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    const FAT_MAGIC: u32 = 0xcafe_babe;
    const FAT_MAGIC_64: u32 = 0xcafe_babf;
    if magic == FAT_MAGIC || magic == FAT_MAGIC_64 {
        let wide = magic == FAT_MAGIC_64;
        let nfat = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let entry_size = if wide { 32 } else { 20 };
        let mut slices = Vec::with_capacity(nfat);
        for i in 0..nfat {
            let base = 8 + i * entry_size;
            if base + entry_size > data.len() {
                return Err(AppError::Internal("verify: truncated fat header".into()));
            }
            let (offset, size) = if wide {
                let off =
                    u64::from_be_bytes(data[base + 8..base + 16].try_into().unwrap()) as usize;
                let sz =
                    u64::from_be_bytes(data[base + 16..base + 24].try_into().unwrap()) as usize;
                (off, sz)
            } else {
                let off =
                    u32::from_be_bytes(data[base + 8..base + 12].try_into().unwrap()) as usize;
                let sz =
                    u32::from_be_bytes(data[base + 12..base + 16].try_into().unwrap()) as usize;
                (off, sz)
            };
            let end = offset
                .checked_add(size)
                .ok_or_else(|| AppError::Internal("verify: fat slice overflow".into()))?;
            if end > data.len() {
                return Err(AppError::Internal("verify: fat slice out of bounds".into()));
            }
            slices.push(&data[offset..end]);
        }
        Ok(slices)
    } else {
        Ok(vec![data])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::test_support::SelfSignedProvider;
    use apple_codesign::macho_builder::MachOBuilder;
    use apple_codesign::{MachoTarget, Platform};

    /// A minimal but signable thin arm64 Mach-O executable with an iOS build
    /// target (the `LC_BUILD_VERSION` load command apple-codesign needs).
    fn minimal_macho() -> Vec<u8> {
        const MH_EXECUTE: u32 = 0x2;
        MachOBuilder::new_aarch64(MH_EXECUTE)
            .macho_target(MachoTarget {
                platform: Platform::IOs,
                minimum_os_version: semver::Version::new(15, 0, 0),
                sdk_version: semver::Version::new(17, 0, 0),
            })
            .write_macho()
            .expect("build minimal macho")
    }

    fn info_plist_xml(bundle_id: &str, exe: &str, name: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>{bundle_id}</string>
  <key>CFBundleExecutable</key><string>{exe}</string>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1.0</string>
  <key>MinimumOSVersion</key><string>15.0</string>
</dict></plist>"#
        )
    }

    /// Build a synthetic `Demo.ipa` with one extension (`Share.appex`) under a
    /// temp dir; returns the ipa path. The temp dir must outlive use.
    fn build_synthetic_ipa(dir: &Path, main_bundle_id: &str) -> PathBuf {
        let app = dir.join("Payload/Demo.app");
        let appex = app.join("PlugIns/Share.appex");
        fs::create_dir_all(&appex).unwrap();

        fs::write(app.join("Demo"), minimal_macho()).unwrap();
        fs::set_permissions(app.join("Demo"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            app.join("Info.plist"),
            info_plist_xml(main_bundle_id, "Demo", "Demo"),
        )
        .unwrap();

        fs::write(appex.join("Share"), minimal_macho()).unwrap();
        fs::set_permissions(appex.join("Share"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            appex.join("Info.plist"),
            info_plist_xml(&format!("{main_bundle_id}.Share"), "Share", "Share"),
        )
        .unwrap();

        // Zip Payload/ into Demo.ipa.
        let ipa = dir.join("Demo.ipa");
        let payload_root = dir.join("Payload");
        let f = File::create(&ipa).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let mut entries = Vec::new();
        collect_bundle_entries(&payload_root, &payload_root, &mut entries).unwrap();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (rel, kind) in entries {
            let name = format!("Payload/{}", rel.to_string_lossy());
            match kind {
                EntryKind::Dir => {
                    zw.add_directory(format!("{name}/"), zip::write::SimpleFileOptions::default())
                        .unwrap();
                }
                EntryKind::Symlink(t) => {
                    zw.add_symlink(name, t, zip::write::SimpleFileOptions::default())
                        .unwrap();
                }
                EntryKind::File(abs, mode) => {
                    zw.start_file(
                        name,
                        zip::write::SimpleFileOptions::default().unix_permissions(mode),
                    )
                    .unwrap();
                    let mut src = File::open(abs).unwrap();
                    std::io::copy(&mut src, &mut zw).unwrap();
                }
            }
        }
        zw.finish().unwrap();
        ipa
    }

    fn names_in_ipa(ipa: &Path) -> Vec<String> {
        let mut a = zip::ZipArchive::new(File::open(ipa).unwrap()).unwrap();
        (0..a.len())
            .map(|i| a.by_index(i).unwrap().name().to_string())
            .collect()
    }

    #[test]
    fn signs_synthetic_ipa_end_to_end_with_keep() {
        let work = tempfile::tempdir().unwrap();
        let ipa = build_synthetic_ipa(work.path(), "com.example.demo");
        let out = work.path().join("signed.ipa");

        let provider = SelfSignedProvider::generate();
        let report =
            sign_ipa(&provider, &ipa, &out, &SignOptions::default()).expect("sign synthetic ipa");

        assert_eq!(report.bundle_id, "com.example.demo");
        assert_eq!(report.original_bundle_id, "com.example.demo");
        assert_eq!(report.app_bundle_name, "Demo.app");
        assert!(out.is_file());

        // The repacked IPA carries the bundle + a fresh _CodeSignature.
        let names = names_in_ipa(&out);
        assert!(names.iter().any(|n| n == "Payload/Demo.app/Demo"));
        assert!(names
            .iter()
            .any(|n| n.starts_with("Payload/Demo.app/_CodeSignature/")));
        // verify_macho_signed already ran inside sign_ipa and would have errored.
    }

    #[test]
    fn replace_strategy_rewrites_main_and_nested_ids() {
        let work = tempfile::tempdir().unwrap();
        let ipa = build_synthetic_ipa(work.path(), "com.example.demo");
        let out = work.path().join("signed.ipa");

        let provider = SelfSignedProvider::generate();
        let opts = SignOptions {
            bundle_id_strategy: BundleIdStrategy::Replace("me.reside.demo".into()),
        };
        let report = sign_ipa(&provider, &ipa, &out, &opts).expect("sign with replace");
        assert_eq!(report.bundle_id, "me.reside.demo");

        // Re-extract the signed IPA and confirm both Info.plists moved.
        let check = work.path().join("check");
        extract_zip(&out, &check).unwrap();
        let main = read_info_plist(&check.join("Payload/Demo.app")).unwrap();
        assert_eq!(
            main.get("CFBundleIdentifier").unwrap().as_string(),
            Some("me.reside.demo")
        );
        let ext = read_info_plist(&check.join("Payload/Demo.app/PlugIns/Share.appex")).unwrap();
        assert_eq!(
            ext.get("CFBundleIdentifier").unwrap().as_string(),
            Some("me.reside.demo.Share")
        );
    }

    #[test]
    fn rejects_zip_slip_entries() {
        let work = tempfile::tempdir().unwrap();
        let evil = work.path().join("evil.ipa");
        {
            let f = File::create(&evil).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            // A path-traversing entry name.
            zw.start_file(
                "Payload/../../escape.txt",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            use std::io::Write;
            zw.write_all(b"pwned").unwrap();
            zw.finish().unwrap();
        }
        let dest = work.path().join("out");
        let err = extract_zip(&evil, &dest).unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn rejects_symlink_escaping_archive_root() {
        let work = tempfile::tempdir().unwrap();
        let evil = work.path().join("evil-symlink.ipa");
        {
            let f = File::create(&evil).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            // A symlink whose *name* is enclosed (passes enclosed_name) but whose
            // *target* climbs out of the extraction root — the symlink zip-slip.
            zw.add_symlink(
                "Payload/escape",
                "../../../../../../etc",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zw.finish().unwrap();
        }
        let dest = work.path().join("out");
        let err = extract_zip(&evil, &dest).unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn symlink_target_within_root_is_allowed() {
        // A relative target that stays inside the root is fine (frameworks use
        // these heavily, e.g. `Current -> A`).
        let root = Path::new("/tmp/extract");
        assert!(symlink_target_stays_within(
            root,
            &root.join("Foo.framework/Versions/Current"),
            "A"
        ));
        // ..but not one that climbs above it, nor an absolute target.
        assert!(!symlink_target_stays_within(
            root,
            &root.join("Foo.framework/x"),
            "../../../../etc/passwd"
        ));
        assert!(!symlink_target_stays_within(
            root,
            &root.join("x"),
            "/etc/passwd"
        ));
    }

    /// Full sign+verify against a real IPA. Gated on `RESIDE_TEST_IPA` so CI
    /// (which has no fixture) skips it. Run locally with:
    ///   RESIDE_TEST_IPA=/path/to/App.ipa cargo test -p reside-core -- --nocapture sign_real_ipa
    #[test]
    fn sign_real_ipa_with_self_signed() {
        let Ok(ipa_path) = std::env::var("RESIDE_TEST_IPA") else {
            eprintln!("skipping: set RESIDE_TEST_IPA to a real .ipa to run this test");
            return;
        };
        let ipa = PathBuf::from(ipa_path);
        let work = tempfile::tempdir().unwrap();
        let out = work.path().join("resigned.ipa");

        let provider = SelfSignedProvider::generate();
        let report =
            sign_ipa(&provider, &ipa, &out, &SignOptions::default()).expect("sign real ipa");

        eprintln!(
            "signed {} ({}) -> {} ({} bytes)",
            report.app_bundle_name,
            report.bundle_id,
            out.display(),
            fs::metadata(&out).unwrap().len()
        );
        assert!(out.is_file());
    }
}
