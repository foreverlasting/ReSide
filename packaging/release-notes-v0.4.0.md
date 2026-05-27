# ReSide v0.4.0

A Linux-first desktop app that signs, sideloads, and **auto-refreshes** iOS apps
on your own iPhone/iPad — the automation, Wi-Fi, and reliability layer over a
proven signer.

## Install

Download `ReSide-0.4.0-linux-x86_64.tar.gz` below, then:

```sh
tar -xzf ReSide-0.4.0-linux-x86_64.tar.gz
cd ReSide-0.4.0-linux-x86_64
./install.sh
```

Runtime deps (Arch/CachyOS):

```sh
sudo pacman -S usbmuxd libimobiledevice webkit2gtk-4.1 libnotify
```

## First-run expectations

- First sign-in downloads a one-time **~150 MB Apple component** from Apple's CDN (needs internet).
- First sign-in asks for a **2FA code** (one-time device trust, not per-app).
- A **keyring** (GNOME Keyring / KWallet) is only needed to save credentials and enable background auto-refresh — otherwise optional.

## Requires

iOS / iPadOS 17.4 or newer (RemoteXPC / RSD transport).

## Verify your download

```sh
sha256sum ReSide-0.4.0-linux-x86_64.tar.gz
# d52449603abde6c1dfa1c718ff97c81cdb9f1a2e28993959d1294e7a70aa50d4
```

## What's inside / source

- **ReSide** (GPL-3.0) — this repo
- **sideloader** (GPL-3.0) — patched fork: https://github.com/foreverlasting/Sideloader (branch `reside-automation`)
- **netmuxd** (LGPL-2.1) — unmodified, [jkcoxson/netmuxd](https://github.com/jkcoxson/netmuxd) @ `1c7dfd1`
