#!/usr/bin/env bash
# Builds Patrone File Manager and installs it for the current user, so it can
# be tried in the dev VM's desktop before it's packaged. No sudo needed.
#
#   ./file-manager/install-dev.sh            install the app, its launcher entry,
#                                            icon and settings schema
#   ./file-manager/install-dev.sh --default  also open folders with it in place of
#                                            Ubuntu's Files (Nautilus), and put it
#                                            in the dock in Files' place
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
make_default=false
for arg in "$@"; do
    case $arg in
        --default) make_default=true ;;
        *) echo "Unknown option: $arg" >&2; exit 1 ;;
    esac
done

cargo install --locked --path "$here" --root ~/.local

apps=~/.local/share/applications
mkdir -p "$apps"
sed "s|^Exec=patrone-file-manager|Exec=$HOME/.local/bin/patrone-file-manager|" \
    "$here/data/org.patrone.FileManager.desktop" > "$apps/org.patrone.FileManager.desktop"
update-desktop-database "$apps" 2>/dev/null || true

icons=~/.local/share/icons/hicolor/scalable/apps
mkdir -p "$icons"
cp "$here/data/org.patrone.FileManager.svg" "$icons/"

# GLib also reads settings schemas from ~/.local/share/glib-2.0/schemas.
schemas=~/.local/share/glib-2.0/schemas
mkdir -p "$schemas"
cp "$here/data/org.patrone.FileManager.gschema.xml" "$schemas/"
glib-compile-schemas "$schemas"
echo "Installed ~/.local/bin/patrone-file-manager, its icon, launcher entry and settings."

if $make_default; then
    # Double-clicking a folder elsewhere, and the taskbar's Files button,
    # open whatever handles inode/directory.
    xdg-mime default org.patrone.FileManager.desktop inode/directory

    # Put File Manager in the dock in place of Ubuntu's Files.
    favorites=$(gsettings get org.gnome.shell favorite-apps)
    favorites=$(python3 -c '
import ast, sys
apps = ast.literal_eval(sys.argv[1].removeprefix("@as "))
apps = ["org.patrone.FileManager.desktop" if app == "org.gnome.Nautilus.desktop" else app for app in apps]
if "org.patrone.FileManager.desktop" not in apps:
    apps.append("org.patrone.FileManager.desktop")
print(apps)
' "$favorites")
    gsettings set org.gnome.shell favorite-apps "$favorites"
    echo "Folders now open in File Manager, and it's in the dock in place of Files."
fi
