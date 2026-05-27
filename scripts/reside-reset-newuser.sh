#!/usr/bin/env bash
# Reset ReSide (and its delegated Sideloader state) to a brand-new-user state.
# Everything is backed up first to a timestamped folder, so this is reversible.
# Does NOT touch device pairing (/var/lib/lockdown) — that step needs sudo and is
# printed at the end for you to run separately.
#
# QUIT ReSide before running this.
set -euo pipefail

TS=$(date +%Y%m%d-%H%M%S)
BACKUP="$HOME/reside-reset-backup-$TS"
mkdir -p "$BACKUP"
echo "==> Backing everything up to: $BACKUP"

# 1. Stop + disable the background refresh agent (user systemd; no sudo).
echo "==> Stopping the background agent…"
systemctl --user disable --now reside-agent.timer reside-agent.service 2>/dev/null || true

# 2. Back up + remove the agent's unit files.
for u in reside-agent.service reside-agent.timer; do
  src="$HOME/.config/systemd/user/$u"
  if [ -f "$src" ]; then cp "$src" "$BACKUP/" && rm -f "$src"; fi
done
systemctl --user daemon-reload 2>/dev/null || true

# 3. Back up + remove ReSide's own state (install history, cached IPAs, locks).
for d in "$HOME/.local/share/reside" "$HOME/.config/reside" "$HOME/.local/state/reside"; do
  if [ -d "$d" ]; then cp -a "$d" "$BACKUP/$(echo "$d" | tr / _)" && rm -rf "$d"; fi
done

# 4. Back up + wipe the shared Sideloader folder: private key/cert, device
#    identity, anisette state, and the downloaded Apple libraries.
#    NOTE: this also resets your GTK Sideloader, since it shares this folder.
if [ -d "$HOME/.config/Sideloader" ]; then
  cp -a "$HOME/.config/Sideloader" "$BACKUP/Sideloader" && rm -rf "$HOME/.config/Sideloader"
fi

# 5. Clear the stored Apple credentials from the keyring (prints no secret).
secret-tool clear application rust-keyring service reside username reside.apple_id     2>/dev/null || true
secret-tool clear application rust-keyring service reside username reside.apple_password 2>/dev/null || true

echo ""
echo "==> Local reset complete. Backup saved at:"
echo "    $BACKUP"
echo ""
echo "==> FINAL STEP (device unpairing — needs sudo, run it yourself):"
echo "    sudo cp /var/lib/lockdown/00008103-001571E136DA001E.plist /var/lib/lockdown/00008150-00065C3C1447801C.plist \"$BACKUP/\" 2>/dev/null"
echo "    sudo rm -f /var/lib/lockdown/00008103-001571E136DA001E.plist /var/lib/lockdown/00008150-00065C3C1447801C.plist"
echo "    (Leave SystemConfiguration.plist alone — it is not a device record.)"
echo ""
echo "To fully undo: stop ReSide, then copy the folders back out of $BACKUP."
