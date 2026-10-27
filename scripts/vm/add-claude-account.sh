#!/usr/bin/env bash
# Adds the "claude" account to the dev VM, so Claude Code can run root
# commands (installing the packages it builds, checking system logs and
# services) without asking for a password: the user's choice, 2026-10-27.
# The account has no password; it signs in over SSH only, with the public
# key given here (Windows: ~/.ssh/patronedev_claude_ed25519, the
# patronedev-claude host in ~/.ssh/config). Run on the VM with sudo:
#
#   sudo bash scripts/vm/add-claude-account.sh ~/claude_ed25519.pub
#
# Undo: sudo rm /etc/sudoers.d/claude && sudo deluser --remove-home claude
set -euo pipefail

key=${1:?Usage: $0 PUBLIC_KEY_FILE}
if [[ $(id -u) -ne 0 ]]; then
    echo "Run it with sudo" >&2
    exit 1
fi
if ! grep -q '^ssh-ed25519 ' "$key"; then
    echo "$key isn't an SSH public key" >&2
    exit 1
fi

if ! id claude > /dev/null 2>&1; then
    adduser --disabled-password --comment "Claude Code (Patrone dev)" claude
fi
# The system journal and logs in /var/log, without sudo
usermod -aG adm,systemd-journal claude
install -d -m 700 -o claude -g claude /home/claude/.ssh
install -m 600 -o claude -g claude "$key" /home/claude/.ssh/authorized_keys

# Checked before it goes in, since a broken rule in /etc/sudoers.d stops
# sudo working for everyone
rule=$(mktemp)
echo 'claude ALL=(ALL) NOPASSWD: ALL' > "$rule"
visudo -cqf "$rule"
install -m 440 -o root -g root "$rule" /etc/sudoers.d/claude
rm -f "$rule"
visudo -cq
echo "Done: claude signs in with its key and uses sudo without a password"
