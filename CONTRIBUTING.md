# Contributing to ReSide

Thanks for helping out. ReSide is a Tauri 2 + Rust desktop app that drives a
forked Sideloader CLI to sign and **auto-refresh** iOS apps on Linux. Read
**[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** first for how it works —
especially the *live-vs-parked* map (`crates/core/src/signer.rs` is the live
signing path; the `signing/` module is an abandoned native attempt).

## Setup

Prerequisites and the full command list live in the README's
**[Building from source](README.md#building-from-source)**: Rust via rustup
(pinned by `rust-toolchain.toml`), Node 18+ / pnpm 10+, and Tauri's Linux system
libraries (WebKitGTK 4.1, GTK 3, libsoup 3, a C toolchain).

```sh
cd crates/tauri-app
pnpm install          # the "ignored build scripts: esbuild" line is expected — see ARCHITECTURE
pnpm tauri:dev        # build and run the full app
```

## Before you open a PR — the four gates must pass

From the repo root:

```sh
cargo test -p reside-core
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
( cd crates/tauri-app && pnpm build )      # the TypeScript gate
```

## Workflow

- Branch off `main`, keep commits focused, and open a PR into `main`.
- **Don't bump pinned dependencies** — including the signer fork's LDC 1.34
  toolchain.
- Diagnostics go through `tracing`, not `println!`.
- Device and Apple-account behavior can only be verified on real Apple hardware —
  if a change touches signing, install, refresh, or device discovery, say so in
  the PR and what still needs a hardware check.

## Where things live

- `crates/core/` — Rust back end (`signer.rs` = live signing path).
- `crates/tauri-app/` — Tauri shell (`src-tauri/`, Rust) + React/TS front end (`src/`).
- `packaging/` — release tarball builder + per-user installer; see
  [`packaging/RELEASING.md`](packaging/RELEASING.md).
- Working with an AI agent? Point it at **[AGENTS.md](AGENTS.md)**.
