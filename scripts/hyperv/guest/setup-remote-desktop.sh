#!/usr/bin/env bash
# Turns on GNOME Remote Desktop so Windows' Remote Desktop Connection (mstsc)
# can connect to this VM with clipboard sharing. This does the same thing as
# Settings → System → Remote Desktop.
#
#   ./setup-remote-desktop.sh sharing   Desktop Sharing: shows the session you're
#                                       logged in to, at the VM's screen size.
#   ./setup-remote-desktop.sh login     Remote Login: a separate session sized
#                                       to the client window (resizing it
#                                       afterwards not yet confirmed).
#                                       Uses sudo. Signing in ends your console
#                                       session, if you have one.
#   ./setup-remote-desktop.sh headless  One sign-in: Remote Desktop Connection
#                                       asks for your username and password and
#                                       opens straight onto your desktop, sized
#                                       to its window. Your desktop session runs
#                                       from boot, with no monitor. Uses sudo.
#                                       Replaces the other two modes, so run it
#                                       over SSH or on the VMConnect console:
#                                       it ends any RDP connection.
#
# Run it as yourself, not with sudo. grdctl asks for the RDP username and
# password to set. For sharing and login they're separate from your Linux
# password; for headless, enter your Linux username and password, which then
# become the ones Remote Desktop Connection asks for. GNOME keeps its own copy
# of them, so after changing your Linux password, run this again. With both
# sharing and login on, Remote Login has port 3389 and Desktop Sharing moves
# to 3390.
set -euo pipefail

if [[ $EUID -eq 0 ]]; then
    echo "Run this as yourself, not with sudo. It uses sudo where it needs to." >&2
    exit 1
fi

# Makes a self-signed TLS certificate, which RDP requires, unless one exists.
# $2 is the command prefix that runs as the certificate's owner.
make_certificate() {
    local dir=$1 as_owner=$2
    if $as_owner test -f "$dir/rdp-tls.crt"; then
        return
    fi
    $as_owner mkdir -p "$dir"
    $as_owner openssl req -x509 -newkey rsa:4096 -nodes -days 3650 -quiet \
        -subj "/CN=$(hostname)" -keyout "$dir/rdp-tls.key" -out "$dir/rdp-tls.crt"
    $as_owner chmod 600 "$dir/rdp-tls.key"
}

here=$(cd "$(dirname "$0")" && pwd)

# Keeps Remote Desktop sessions from locking or blanking (rdp-no-lock.sh),
# starting with the next login.
install_no_lock() {
    install -Dm755 "$here/rdp-no-lock.sh" ~/.local/bin/patrone-rdp-no-lock
    mkdir -p ~/.config/autostart
    cat > ~/.config/autostart/patrone-rdp-no-lock.desktop << EOF
[Desktop Entry]
Type=Application
Name=No screen lock over Remote Desktop
Exec=$HOME/.local/bin/patrone-rdp-no-lock
NoDisplay=true
EOF
}

setup_sharing() {
    local dir=~/.local/share/gnome-remote-desktop
    make_certificate "$dir" ""
    grdctl rdp set-tls-cert "$dir/rdp-tls.crt"
    grdctl rdp set-tls-key "$dir/rdp-tls.key"
    grdctl rdp disable-view-only
    echo "Choose a username and password for Desktop Sharing:"
    grdctl rdp set-credentials
    grdctl rdp enable
    systemctl --user enable gnome-remote-desktop.service
    systemctl --user restart gnome-remote-desktop.service
    grdctl status
}

setup_login() {
    local dir=/var/lib/gnome-remote-desktop/certificates
    make_certificate "$dir" "sudo -u gnome-remote-desktop"
    sudo grdctl --system rdp set-tls-cert "$dir/rdp-tls.crt"
    sudo grdctl --system rdp set-tls-key "$dir/rdp-tls.key"
    echo "Choose a username and password for Remote Login (separate from Desktop"
    echo "Sharing's). After connecting, you sign in at the login screen with your"
    echo "normal Linux account."
    sudo grdctl --system rdp set-credentials
    sudo grdctl --system rdp enable
    sudo systemctl enable gnome-remote-desktop.service
    sudo systemctl restart gnome-remote-desktop.service
    sudo grdctl --system status
    install_no_lock
}

# This user's open desktop (not text console) sessions.
desktop_sessions() {
    local id
    for id in $(loginctl list-sessions --no-legend | awk -v user="$USER" '$3 == user { print $1 }'); do
        case $(loginctl show-session "$id" -p Type --value) in
            wayland | x11) echo "$id" ;;
        esac
    done
}

setup_headless() {
    local session="gnome-headless-session@$USER.service"
    # GNOME runs one desktop session per user, so the headless one can't
    # start beside another.
    if ! systemctl is-active --quiet "$session"; then
        local others
        others=$(desktop_sessions)
        if [[ -n $others ]]; then
            echo "Log out of your desktop sessions first (logind sessions: $(echo $others))." >&2
            echo "loginctl terminate-session ID ends one, without saving its work." >&2
            exit 1
        fi
    fi

    # Remote Login and Desktop Sharing hold the RDP ports, and the headless
    # server conflicts with Desktop Sharing.
    sudo systemctl disable --now gnome-remote-desktop.service
    systemctl --user disable --now gnome-remote-desktop.service

    local dir=~/.local/share/gnome-remote-desktop
    make_certificate "$dir" ""
    grdctl --headless rdp set-tls-cert "$dir/rdp-tls.crt"
    grdctl --headless rdp set-tls-key "$dir/rdp-tls.key"
    grdctl --headless rdp set-port 3389
    grdctl --headless rdp disable-view-only
    echo "Enter your Linux username and password. Remote Desktop Connection will"
    echo "ask for these, and they open your desktop with no second sign-in."
    grdctl --headless rdp set-credentials
    grdctl --headless rdp enable
    systemctl --user enable gnome-remote-desktop-headless.service

    # Nothing unlocks this session when it starts, so its lock screen would
    # be a second sign-in.
    install_no_lock

    sudo systemctl enable --now "$session"
    grdctl --headless status
}

case ${1:-} in
    sharing) setup_sharing ;;
    login) setup_login ;;
    headless) setup_headless ;;
    *)
        echo "Usage: $0 sharing|login|headless" >&2
        exit 1
        ;;
esac

echo
echo "Connect from Windows with: mstsc /v:$(hostname).local"
