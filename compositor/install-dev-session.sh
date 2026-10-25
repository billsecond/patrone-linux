#!/usr/bin/env bash
# Builds the compositor and adds "Patrone" to the login screen's session
# list, so it can be chosen at login (the gear button on GDM). Uses sudo.
#
#   ./compositor/install-dev-session.sh            install or update
#   ./compositor/install-dev-session.sh --remove   take it off the login screen
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)

if [[ ${1:-} == --remove ]]; then
    sudo rm -f /usr/share/wayland-sessions/patrone.desktop \
        /usr/local/bin/patrone-session /usr/local/bin/patrone-compositor
    echo "Removed the Patrone session."
    exit 0
fi

cargo build --release --manifest-path "$here/Cargo.toml"
sudo install -m 755 "$here/target/release/patrone-compositor" /usr/local/bin/
sudo install -m 755 "$here/data/patrone-session" /usr/local/bin/
sudo install -m 644 "$here/data/patrone.desktop" /usr/share/wayland-sessions/
echo "Installed. Choose \"Patrone\" with the gear button on the login screen."
echo "Its log: ~/.local/state/patrone-session.log"
