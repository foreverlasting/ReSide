# Dependency License Inventory

ReSide is MIT-licensed. All bundled dependencies are permissively licensed and
MIT-compatible. This file is kept current as dependencies are added.

## Rust crates (current as of Phase 0)

| Crate | License | Notes |
|-------|---------|-------|
| tauri / tauri-plugin-* | MIT / Apache-2.0 | App framework + official plugins |
| sqlx | MIT / Apache-2.0 | SQLite access |
| keyring | MIT / Apache-2.0 | Secret Service backend (sync-secret-service + crypto-rust) |
| fs2 | MIT / Apache-2.0 | Process file lock |
| dirs | MIT / Apache-2.0 | XDG path resolution |
| sha2 / hex | MIT / Apache-2.0 | Content-addressed IPA store |
| serde / serde_json | MIT / Apache-2.0 | Serialization |
| thiserror | MIT / Apache-2.0 | Error taxonomy |
| tracing / tracing-subscriber | MIT | Structured diagnostics |
| tokio | MIT | Async runtime |

## Planned dependencies (land with later phases)

| Crate | License | Phase |
|-------|---------|-------|
| idevice | MIT | 1 (device transport / install) |
| mdns-sd | MIT / Apache-2.0 | 1 (Wi-Fi discovery) |
| apple-codesign | MPL-2.0 | 2 (IPA signing) |
| omnisette / icloud-auth / apple-dev-apis | MPL-2.0 | 2 (Apple auth, by git rev) |
| libloading | ISC | 2 (ADI FFI) |
| zip / plist | MIT / Apache-2.0 | 2 (IPA pipeline) |

## NOT shipped: Apple ADI libraries

`libstoreservicescore.so` and `libCoreADI.so` are **Apple-proprietary and
non-redistributable**. ReSide never bundles, commits, or ships them. They are
downloaded and extracted **on the user's machine** at setup time from a
user-initiated Apple Music APK download, into the app data directory only. The
repository, release artifacts, and CI never contain them. As a consequence the
free-signing path is not pure-Rust (runtime FFI via `libloading`).
