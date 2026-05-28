#!/usr/bin/env bash
#
# install.sh — install (or remove) ReSide for the current user.
#
# Installs into your home directory only — no root, no system files:
#
#   ~/.local/lib/reside/      reside, sideloader, netmuxd, reside.png  (kept together)
#   ~/.local/bin/reside       symlink onto the app  (so `reside` works from a terminal)
#   ~/.local/share/applications/dev.reside.app.desktop   (so it shows in your menu)
#   ~/.local/share/icons/hicolor/{scalable,32x32,128x128,256x256,512x512}/apps/reside.{svg,png}
#       (multiple sizes + an SVG so the desktop environment can pick a crisp
#       rendering at any display size — apps menu, taskbar, alt-tab, etc.)
#
# The three binaries MUST stay in the same folder: ReSide finds the signer and
# Wi-Fi helper sitting beside itself. The launcher symlink resolves back to that
# folder, so menu launches and terminal launches behave identically.
#
# Usage:
#   ./install.sh              install for the current user
#   ./install.sh --uninstall  remove everything listed above
#
set -euo pipefail

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PREFIX="${PREFIX:-$HOME/.local}"
LIBDIR="$PREFIX/lib/reside"
BINDIR="$PREFIX/bin"
APPDIR="$PREFIX/share/applications"
HICOLOR="$PREFIX/share/icons/hicolor"

# Where each source icon goes inside the hicolor theme. The DE picks the
# best-matching size on demand; the SVG covers anything the PNG sizes don't.
# Format: "<hicolor-subdir>:<source-filename>" — keep both columns aligned with
# the files staged by build-tarball.sh into the tarball's `icons/` dir.
ICON_INSTALLS=(
  "scalable/apps:icon.svg"
  "32x32/apps:32x32.png"
  "128x128/apps:128x128.png"
  "256x256/apps:128x128@2x.png"
  "512x512/apps:icon.png"
)

DESKTOP_FILE="$APPDIR/dev.reside.app.desktop"
LAUNCHER="$BINDIR/reside"

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }

# Refresh the desktop-entry + icon caches so the menu picks up our entry and
# icon immediately, without a re-login. Three caches; not all DEs have all
# three, so each is best-effort.
#
#   update-desktop-database  — the application database (GNOME, XFCE, …).
#   gtk-update-icon-cache    — GTK's hicolor icon-name → file index.
#   kbuildsycoca6/5          — KDE Plasma's separate app database. KDE does
#                              not consult GTK's icon cache, so this is the
#                              one that actually makes the icon show up under
#                              Plasma. Prefer v6 (Plasma 6, current); fall
#                              back to v5.
refresh_desktop_caches() {
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPDIR"   2>/dev/null || true
  command -v gtk-update-icon-cache   >/dev/null 2>&1 && gtk-update-icon-cache --force --quiet "$HICOLOR" 2>/dev/null || true
  if   command -v kbuildsycoca6 >/dev/null 2>&1; then kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
  elif command -v kbuildsycoca5 >/dev/null 2>&1; then kbuildsycoca5 --noincremental >/dev/null 2>&1 || true
  fi
}

uninstall() {
  say "Removing ReSide"
  rm -rf "$LIBDIR"
  rm -f  "$LAUNCHER" "$DESKTOP_FILE"
  for entry in "${ICON_INSTALLS[@]}"; do
    subdir="${entry%%:*}"
    src="${entry##*:}"
    rm -f "$HICOLOR/$subdir/reside.${src##*.}"
  done
  refresh_desktop_caches
  say "Removed. (Your signed apps, credentials, and app data under ~/.local/share/reside were left untouched.)"
  exit 0
}

[ "${1:-}" = "--uninstall" ] && uninstall

# --- install ----------------------------------------------------------------
for b in reside sideloader netmuxd; do
  [ -f "$SRC_DIR/$b" ] || { printf 'error: missing %s next to this script — is the tarball intact?\n' "$b" >&2; exit 1; }
done
[ -d "$SRC_DIR/icons" ] || { printf 'error: missing icons/ next to this script — is the tarball intact?\n' >&2; exit 1; }

say "Installing ReSide into $PREFIX"
mkdir -p "$LIBDIR" "$BINDIR" "$APPDIR"

install -m 0755 "$SRC_DIR/reside"          "$LIBDIR/reside"
install -m 0755 "$SRC_DIR/sideloader"      "$LIBDIR/sideloader"
install -m 0755 "$SRC_DIR/netmuxd"         "$LIBDIR/netmuxd"
install -m 0644 "$SRC_DIR/icons/icon.png"  "$LIBDIR/reside.png"

# Icons into the hicolor theme. Each install creates its size subdir if absent.
for entry in "${ICON_INSTALLS[@]}"; do
  subdir="${entry%%:*}"
  src="${entry##*:}"
  mkdir -p "$HICOLOR/$subdir"
  install -m 0644 "$SRC_DIR/icons/$src" "$HICOLOR/$subdir/reside.${src##*.}"
done

ln -sf "$LIBDIR/reside" "$LAUNCHER"

# desktop entry: point Exec at the real installed binary so menu launches don't
# depend on ~/.local/bin being on the session PATH. Icon= uses an **absolute
# path** rather than the icon-theme name `reside`, because KDE Plasma's
# KIconLoader display-time cache doesn't always pick up new themed icons in
# the user-local hicolor dir even after `kbuildsycoca` rebuild — the menu ends
# up showing a generic glyph. An absolute path bypasses theme lookup entirely
# and works on every DE (GNOME, Plasma, XFCE, …).
sed -e "s|__EXEC__|$LIBDIR/reside|" \
    -e "s|__ICON__|$HICOLOR/scalable/apps/reside.svg|" \
    "$SRC_DIR/reside.desktop" > "$DESKTOP_FILE"
chmod 0644 "$DESKTOP_FILE"

refresh_desktop_caches

say "Installed."
echo "  Launch from your app menu (\"ReSide\"), or run: reside"

# nudge if ~/.local/bin isn't on PATH (terminal launch won't find `reside` otherwise)
case ":$PATH:" in
  *":$BINDIR:"*) : ;;
  *) echo
     echo "  Note: $BINDIR is not on your PATH, so the \`reside\` command won't be found"
     echo "  in a terminal. The app-menu entry still works. To enable the command, add:"
     echo "      set -gx PATH $BINDIR \$PATH        # fish"
     echo "      export PATH=\"$BINDIR:\$PATH\"       # bash/zsh"
     ;;
esac
# Tray runtime dep check. The app launches fine without libayatana-appindicator3
# (lib.rs wraps the tray init in catch_unwind), but the tray icon won't appear.
# Friendly note + per-distro install command if it's missing.
if ! ldconfig -p 2>/dev/null | grep -q "libayatana-appindicator3\|libappindicator3"; then
  echo
  echo "  Note: libayatana-appindicator3 not found — the system tray icon will be"
  echo "  disabled. The app launches and the menu/window all work; this only"
  echo "  affects the tray surface. To enable the tray, install the lib:"
  if   command -v pacman >/dev/null 2>&1; then echo "      sudo pacman -S libayatana-appindicator"
  elif command -v dnf    >/dev/null 2>&1; then echo "      sudo dnf install libayatana-appindicator-gtk3"
  elif command -v apt    >/dev/null 2>&1; then echo "      sudo apt install libayatana-appindicator3-1"
  else                                         echo "      (search your distro's packages for libayatana-appindicator3)"
  fi
fi

echo
echo "  First sign-in downloads a one-time ~150 MB Apple component and asks for a 2FA code — see README."
echo "  To remove later:  ~/.local/lib/reside  →  re-run this script with --uninstall, or just delete that folder."
