# ReSide v0.5.3

A Linux-first desktop app that signs, sideloads, and **auto-refreshes** iOS apps
on your own iPhone/iPad — the automation, Wi-Fi, and reliability layer over a
proven signer.

A small fix release. The signer, Wi-Fi, and auto-refresh engine are unchanged
from v0.5.2.

## What's new since v0.5.2

- **Version badge shows the real version.** The sidebar badge was hardcoded
  "v0.5" and never reflected patch releases; it now reads the actual app version
  (e.g. "v0.5.3") and tracks every future bump automatically.

## Install

Download `ReSide-0.5.3-linux-x86_64.tar.gz` below, then:

```sh
tar -xzf ReSide-0.5.3-linux-x86_64.tar.gz
cd ReSide-0.5.3-linux-x86_64
./install.sh
```

Runtime deps (Arch / CachyOS):

```sh
sudo pacman -S usbmuxd libimobiledevice webkit2gtk-4.1 libnotify libayatana-appindicator
```

`libayatana-appindicator` is optional — it only enables the tray icon. The app
launches and the menu / window all work without it.

## First-run expectations

- First sign-in downloads a one-time **~150 MB Apple component** from Apple's CDN (needs internet).
- First sign-in asks for a **2FA code** (one-time device trust, not per-app).
- A **keyring** (GNOME Keyring / KWallet) is only needed to save credentials and enable background auto-refresh — otherwise optional.

## Requires

Recommended: iOS / iPadOS 17.4 or newer (the version validated on hardware).
Older 17.x may work but is untested.

## Verify your download

The matching SHA-256 is attached as `ReSide-0.5.3-linux-x86_64.tar.gz.sha256`:

```sh
sha256sum -c ReSide-0.5.3-linux-x86_64.tar.gz.sha256
```

## What's inside / source

- **ReSide** (GPL-3.0) — this repo
- **sideloader** (GPL-3.0) — patched fork: https://github.com/foreverlasting/Sideloader (branch `reside-automation`)
- **netmuxd** (LGPL-2.1) — unmodified, [jkcoxson/netmuxd](https://github.com/jkcoxson/netmuxd) @ `1c7dfd1`
