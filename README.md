# ReSide

A Linux-first desktop app for signing, sideloading, and **auto-refreshing** iOS
apps on your own iPhone or iPad — a native alternative to tools like Sideloadly.

ReSide doesn't reimplement Apple's signing stack. It's the **automation, Wi-Fi,
and reliability layer** on top of a proven signer (a fork of
[Dadoum's Sideloader](https://github.com/Dadoum/Sideloader)), built to fix three
things those tools leave to you:

- **Auto-refresh** — free Apple-ID signatures expire after 7 days. ReSide
  re-signs and reinstalls *before* they lapse, in the background, so your apps
  keep working without you touching anything.
- **Wi-Fi** — sign and install over the network, no cable required after the
  first pairing.
- **Reliable signing** — driven through a single, validated signer with a
  one-click "Refresh now" recovery when Apple throttles.

> **Requires iOS / iPadOS 17.4 or newer.** ReSide uses the modern RemoteXPC /
> RSD transport; it does not support older lockdown-only devices.

## Before you start — three things to expect

1. **First sign-in downloads a ~150 MB Apple component (one time).** To sign
   apps with a free Apple ID, Apple's own provisioning library is required.
   ReSide downloads it on first use straight from Apple's CDN (it is *not*
   bundled with ReSide and never committed to this repo — see
   [LICENSES.md](LICENSES.md)). You'll need an internet connection for that
   first run.
2. **First sign-in needs a 2FA code.** Signing in on a new device is a
   one-time Apple device-trust step — you'll be prompted for the
   two-factor code sent to your Apple devices. It's per-device, not per-app, so
   you only do it once.
3. **A keyring is optional.** A system keyring (GNOME Keyring / KWallet) is only
   needed if you want ReSide to *save* your Apple-ID credentials and run the
   background auto-refresh agent. Without one, you can still sign and install —
   ReSide just asks for your password each session (or holds it only in memory
   for the current run).

## Install

Download `ReSide-<version>-linux-x86_64.tar.gz` from the
[latest release](../../releases/latest), then:

```sh
tar -xzf ReSide-*-linux-x86_64.tar.gz
cd ReSide-*-linux-x86_64
./install.sh
```

This installs into your home directory only (no root): the app and its two
helper binaries go in `~/.local/lib/reside/`, with a launcher and a menu entry.
Launch it from your app menu ("ReSide") or run `reside`. To remove it later,
re-run `./install.sh --uninstall`.

You can also just run `./reside` straight from the extracted folder without
installing — the helpers sit beside it and are found automatically.

### Runtime requirements

- `usbmuxd` and `libimobiledevice` — talk to the device
- `webkit2gtk-4.1` and `libnotify` — the app UI and notifications

On Arch / CachyOS:

```sh
sudo pacman -S usbmuxd libimobiledevice webkit2gtk-4.1 libnotify
```

### What's in the tarball

| File | License | What it is |
|------|---------|------------|
| `reside` | GPL-3.0 | The app itself |
| `sideloader` | GPL-3.0 | The forked Dadoum signer ReSide drives |
| `netmuxd` | LGPL-2.1 | The on-demand bridge that enables Wi-Fi install/refresh |

The two helpers are separate processes ReSide spawns, shipped prebuilt for
convenience. Their sources are linked under [Building from source](#building-from-source)
below; both are free software.

## Building from source

ReSide itself builds with a standard Rust + Node toolchain:

```sh
git clone <this repo> && cd <repo>/crates/tauri-app
pnpm install
pnpm tauri:dev          # run the full app
cargo test -p reside-core
cargo build --workspace
```

To produce a release tarball like the one above, you need the two helper
binaries built first, then run the packaging script:

- **netmuxd** ([source](https://github.com/jkcoxson/netmuxd)) — one command:
  `cargo build --release`. Shipped unmodified at commit `1c7dfd1`.
- **sideloader** — the patched fork ReSide drives
  ([source](https://github.com/foreverlasting/Sideloader), branch
  `reside-automation`). Its build pins a specific D toolchain (LDC 1.34) and is
  the fiddly part; see the fork's build notes.

```sh
# from the repo root, with the helpers built and discoverable
RESIDE_SIDELOADER_SRC=/path/to/sideloader \
RESIDE_NETMUXD_SRC=/path/to/netmuxd \
  packaging/build-tarball.sh
# → target/release-tarball/ReSide-<version>-linux-x86_64.tar.gz
```

## Repository layout

```
crates/
├── core/            reside-core — orchestration, signing, transport, refresh
└── tauri-app/       desktop app
    ├── src/         React + TypeScript + Tailwind frontend
    └── src-tauri/   Tauri 2 shell (Rust)
packaging/           release tarball builder + per-user installer
```

See [`plan.md`](plan.md) for the full architecture.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).

Copyright (C) 2026 ReSide contributors. This program is free software: you can
redistribute it and/or modify it under the terms of the GNU General Public
License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

Dependency inventory in [LICENSES.md](LICENSES.md). Note: ReSide never ships
Apple's ADI libraries — they're downloaded on your machine at first use, never
bundled here.
