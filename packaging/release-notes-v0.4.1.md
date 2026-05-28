# ReSide v0.4.1

A Linux-first desktop app that signs, sideloads, and **auto-refreshes** iOS apps
on your own iPhone/iPad — the automation, Wi-Fi, and reliability layer over a
proven signer.

Hardening point-release that supersedes the never-published v0.4.0 draft. All
four fixes below were prompted by hardware testing on KDE Plasma / CachyOS /
Wayland.

## What's fixed since v0.4.0

- **App now launches from the apps menu on Wayland (#5).** The released binary
  bakes in the WebKit/Wayland workaround that the dev script already had — no
  more silent "menu launch does nothing" on CachyOS and similar Wayland stacks.
- **Apps-menu icon now resolves on KDE Plasma (#7, #8).** Installs the icon at
  multiple hicolor sizes plus an SVG, refreshes the GTK icon cache **and** KDE's
  sycoca app database, and writes an absolute `Icon=` path into the `.desktop`
  file so KDE's KIconLoader can't miss it.
- **System tray icon (#7).** Left-click toggles the window; right-click gives
  Show / Quit. Optional — requires `libayatana-appindicator3`; the install
  script tells you the package name for your distro if it's missing.
- **App still launches without the tray lib (#8).** If
  `libayatana-appindicator3` isn't installed, the tray init's panic is now
  caught and the app continues without a tray surface instead of aborting at
  startup.
- **Pre-public polish (#6).** Mock design-gallery screens are clearly labelled
  as previews, parked native-signing modules now each carry a PARKED banner,
  benign `mdns_sd` shutdown ERROR noise is quieted, README iOS claim softened
  to "17.4+ recommended (validated); older may work but untested," stale
  Windows `.ico` dropped.

## Install

Download `ReSide-0.4.1-linux-x86_64.tar.gz` below, then:

```sh
tar -xzf ReSide-0.4.1-linux-x86_64.tar.gz
cd ReSide-0.4.1-linux-x86_64
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

```sh
sha256sum ReSide-0.4.1-linux-x86_64.tar.gz
# (sha printed by build-tarball.sh after build)
```

## What's inside / source

- **ReSide** (GPL-3.0) — this repo
- **sideloader** (GPL-3.0) — patched fork: https://github.com/foreverlasting/Sideloader (branch `reside-automation`)
- **netmuxd** (LGPL-2.1) — unmodified, [jkcoxson/netmuxd](https://github.com/jkcoxson/netmuxd) @ `1c7dfd1`
