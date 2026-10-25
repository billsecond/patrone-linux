#!/usr/bin/env bash
# Lets Hyper-V's console window (VMConnect) open an enhanced session, with
# clipboard sharing, into this VM's GNOME desktop.
#
# In enhanced-session mode Hyper-V connects to vsock port 3389 inside the
# guest. GNOME Remote Desktop only listens on TCP, so a small socat service
# bridges the two:
#
#   VMConnect --hv_sock--> vsock :3389 --socat--> TCP 127.0.0.1:3389 (GNOME Remote Desktop)
#
# Before running this, turn on Desktop Sharing with setup-remote-desktop.sh.
#
# Run in the VM:   sudo ./scripts/hyperv/guest/setup-enhanced-session.sh
# Then on Windows: Set-VM -VMName PatroneDev -EnhancedSessionTransportType HvSocket
#                  (with the VM shut down), start it, and connect with VMConnect.
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "Run with sudo." >&2
    exit 1
fi
if [[ $(systemd-detect-virt) != microsoft ]]; then
    echo "This isn't a Hyper-V guest." >&2
    exit 1
fi
if ! command -v socat > /dev/null; then
    apt-get install -y socat
fi

here=$(cd "$(dirname "$0")" && pwd)
install -m 644 "$here/patrone-hyperv-rdp-bridge.service" /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now patrone-hyperv-rdp-bridge.service
systemctl --no-pager status patrone-hyperv-rdp-bridge.service
