# ReSide v0.5.2

A Linux-first desktop app that signs, sideloads, and **auto-refreshes** iOS apps
on your own iPhone/iPad — the automation, Wi-Fi, and reliability layer over a
proven signer.

A correctness + robustness release: better multi-device handling and a startup
that can no longer fail invisibly. The signer and auto-refresh engine are
unchanged from v0.5.1.

## What's new since v0.5.1

- **Multi-device correctness + Wi-Fi ladder remodel (§7k).** With more than one
  device connected, identity (name / iOS version / model) is now resolved
  per-device instead of occasionally borrowing another device's details over
  Wi-Fi, and tunnel state is tracked per-UDID so moving a device between USB and
  Wi-Fi re-establishes cleanly rather than reusing a stale tunnel.
- **Startup never fails silently.** If ReSide can't open its data directory or
  database at launch, it now logs the reason, writes a `logs/startup-error.log`
  breadcrumb, and shows a native error dialog instead of exiting with no window —
  which previously looked like "nothing happens" when launched from the apps
  menu.
- **Clear message when your data is newer than the app.** If a database created
  by a newer ReSide is opened by an older build, you now get "update ReSide to
  the latest release" instead of an opaque "migration error". (This is also the
  fix for the older v0.5.1 download refusing to launch against a data dir a newer
  build had already upgraded.)

## Install

Download `ReSide-0.5.2-linux-x86_64.tar.gz` below, then:

```sh
tar -xzf ReSide-0.5.2-linux-x86_64.tar.gz
cd ReSide-0.5.2-linux-x86_64
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

The matching SHA-256 is attached as `ReSide-0.5.2-linux-x86_64.tar.gz.sha256`:

```sh
sha256sum -c ReSide-0.5.2-linux-x86_64.tar.gz.sha256
```

## What's inside / source

- **ReSide** (GPL-3.0) — this repo
- **sideloader** (GPL-3.0) — patched fork: https://github.com/foreverlasting/Sideloader (branch `reside-automation`)
- **netmuxd** (LGPL-2.1) — unmodified, [jkcoxson/netmuxd](https://github.com/jkcoxson/netmuxd) @ `1c7dfd1`
