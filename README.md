# ReSide

A Linux-first desktop app for signing, sideloading, and **auto-refreshing** iOS
apps on your own iPhone/iPad — a native alternative to tools like Sideloadly.

> **Requires iOS / iPadOS 17.4 or newer.** ReSide uses the RemoteXPC / RSD
> transport and does not support older lockdown-only devices in v1.

The headline feature is background Wi-Fi refresh: re-sign and reinstall before
Apple's 7-day free-account signing window expires, without plugging in.

## Status

Early development. The frontend (all 6 flows) and the core scaffold are in
place; device, signing, install, and refresh logic land phase by phase. See
[`plan.md`](plan.md) for the full architecture and phased roadmap.

## Repository layout

```
crates/
├── core/            reside-core — orchestration, signing, transport, refresh
└── tauri-app/       desktop app
    ├── src/         React + TypeScript + Tailwind frontend
    └── src-tauri/   Tauri 2 shell (Rust)
```

## Development

Prerequisites: a Rust toolchain, Node + pnpm, and the Tauri Linux system deps
(`webkit2gtk-4.1`, `libsoup-3.0`, plus `usbmuxd`, `libimobiledevice`,
`libnotify` at runtime).

```sh
cd crates/tauri-app
pnpm install
pnpm tauri dev        # full desktop app (Vite HMR + cargo watch)
pnpm dev              # frontend only, in a browser

# Rust
cargo test -p reside-core              # headless core tests
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## License

MIT — see [LICENSE](LICENSE). Dependency inventory in
[LICENSES.md](LICENSES.md). Note: ReSide never ships Apple's ADI libraries; see
LICENSES.md for details.
