#!/bin/sh
# Runs at each GNOME login, from ~/.config/autostart (setup-remote-desktop.sh
# installs it). In a Remote Desktop session (Remote Login or headless, which
# have no seat) it turns off the screen lock and screen blanking, which nobody
# expects in a remote window. On the VM's own screen it puts GNOME's defaults
# back. GNOME keeps these settings per user, not per session, and runs one
# desktop session per user at a time, so the session that starts decides.
session=$(loginctl show-user "$USER" -p Display --value)
[ -n "$session" ] || exit 0

if [ -z "$(loginctl show-session "$session" -p Seat --value)" ]; then
    gsettings set org.gnome.desktop.screensaver lock-enabled false
    gsettings set org.gnome.desktop.session idle-delay 0
else
    gsettings reset org.gnome.desktop.screensaver lock-enabled
    gsettings reset org.gnome.desktop.session idle-delay
fi
