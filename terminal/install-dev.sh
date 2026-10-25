#!/usr/bin/env bash
# Builds Patrone Terminal and installs it for the current user, so it can be
# tried in the dev VM's GNOME session before it's packaged. No sudo needed.
#
#   ./terminal/install-dev.sh            install the app and its launcher entry
#   ./terminal/install-dev.sh --default  also make it GNOME's default terminal
#   ./terminal/install-dev.sh --zshrc    also install Patrone's zshrc as ~/.zshrc
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
make_default=false
install_zshrc=false
for arg in "$@"; do
    case $arg in
        --default) make_default=true ;;
        --zshrc) install_zshrc=true ;;
        *) echo "Unknown option: $arg" >&2; exit 1 ;;
    esac
done

cargo install --locked --path "$here" --root ~/.local

apps=~/.local/share/applications
mkdir -p "$apps"
sed "s|^Exec=patrone-terminal|Exec=$HOME/.local/bin/patrone-terminal|" \
    "$here/data/org.patrone.Terminal.desktop" > "$apps/org.patrone.Terminal.desktop"
update-desktop-database "$apps" 2>/dev/null || true
echo "Installed ~/.local/bin/patrone-terminal and its launcher entry."

if $make_default; then
    # GNOME opens terminals through xdg-terminal-exec, which reads this list.
    echo org.patrone.Terminal.desktop > ~/.config/xdg-terminals.list
    echo "Patrone Terminal is now the default terminal (~/.config/xdg-terminals.list)."
fi

if $install_zshrc; then
    if [[ -e ~/.zshrc ]]; then
        cp ~/.zshrc ~/.zshrc.before-patrone
        echo "Saved the old ~/.zshrc as ~/.zshrc.before-patrone."
    fi
    cp "$here/../zsh/zshrc" ~/.zshrc
    echo "Installed ~/.zshrc. To make zsh your login shell: chsh -s /usr/bin/zsh"
fi
