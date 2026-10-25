#!/usr/bin/env bash
# Installs everything the Patrone Linux dev VM needs to build packages and ISOs.
# Run once on a fresh Ubuntu 26.04 desktop install: sudo ./scripts/bootstrap-dev-vm.sh
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "Run with sudo." >&2
    exit 1
fi

apt-get update

# Remote access and source control
apt-get install -y openssh-server git gh curl

# Debian/Ubuntu packaging
apt-get install -y build-essential devscripts debhelper equivs \
    git-buildpackage sbuild mmdebstrap ubuntu-dev-tools reprepro

# ISO building
apt-get install -y debootstrap squashfs-tools xorriso mtools dosfstools

# Booting test ISOs inside this VM (nested virtualization is enabled on the host)
apt-get install -y qemu-system-x86 qemu-utils ovmf

# Desktop development: Wayland compositor (Smithay) and GTK4 shell libraries.
# Rust itself is installed per user with rustup, not here.
apt-get install -y pkg-config clang meson ninja-build \
    libwayland-dev wayland-protocols libxkbcommon-dev libinput-dev libudev-dev \
    libseat-dev libdrm-dev libgbm-dev libegl-dev libpixman-1-dev libdisplay-info-dev \
    xwayland libgtk-4-dev libgtk4-layer-shell-dev

# Hyper-V integration: lets the Windows host see this VM's IP address
apt-get install -y linux-cloud-tools-virtual || true

systemctl enable --now ssh

# Let the desktop user run QEMU with KVM acceleration
if [[ -n "${SUDO_USER:-}" ]]; then
    usermod -aG kvm "$SUDO_USER"
fi

echo "Done. Log out and back in so the kvm group takes effect."
