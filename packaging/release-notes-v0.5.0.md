# ReSide v0.5.0

A Linux-first desktop app that signs, sideloads, and **auto-refreshes** iOS apps
on your own iPhone/iPad — the automation, Wi-Fi, and reliability layer over a
proven signer.

A UX-focused release: navigation is now persistent across the whole app, and the
device, system, and history surfaces were rebuilt so nothing hides behind a
full-screen takeover. The signer, Wi-Fi, and auto-refresh engine are unchanged
from v0.4.1.

## What's new since v0.4.1

- **Persistent sidebar (§7h).** The sidebar, window chrome, and toolbar now stay
  put across every surface — only the main pane swaps. Switching between Apps,
  Devices, Activity, System, and Settings no longer feels like jumping between
  different apps.
- **Devices is now a real device manager (§7e/§7f).** It's an in-shell pane, not a
  full-screen pairing wizard. One device fills the pane (a switcher appears only
  when you have more than one), and a **connection ladder** — Paired → Developer
  Mode → Secure tunnel → Wi-Fi refresh — shows exactly where setup is and what to
  fix next, with later steps locked behind the current blocker. Pairing itself is
  now a focused "Trust this computer" dialog.
- **New System view.** Review the dependency / backend status check any time, not
  just during first-run onboarding.
- **New Activity view (§7a).** A running log of installs and background refreshes
  with severity, operation, and timing.
- **Wi-Fi connect is gated on pairing (§7i).** A reachable-but-unpaired iPhone now
  nudges you to plug in over USB once, instead of offering a connect button that
  can't work yet.
- **Dark-mode polish (§7j).** Modals, scrims, and native form controls now follow
  the Dracula theme correctly instead of flashing light.
- **Cleanup.** Dead/no-op controls removed or wired up, a single shared Apple-ID
  entry component, and the light/dark theme choice now persists across launches.

## Install

Download `ReSide-0.5.0-linux-x86_64.tar.gz` below, then:

```sh
tar -xzf ReSide-0.5.0-linux-x86_64.tar.gz
cd ReSide-0.5.0-linux-x86_64
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
sha256sum ReSide-0.5.0-linux-x86_64.tar.gz
# 1a92ba014f80793deecfcbc1f62c0b5cbd81ba62bcc7a7153b6fc1212204dea1
```

## What's inside / source

- **ReSide** (GPL-3.0) — this repo
- **sideloader** (GPL-3.0) — patched fork: https://github.com/foreverlasting/Sideloader (branch `reside-automation`)
- **netmuxd** (LGPL-2.1) — unmodified, [jkcoxson/netmuxd](https://github.com/jkcoxson/netmuxd) @ `1c7dfd1`
