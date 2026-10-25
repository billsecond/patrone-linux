#!/usr/bin/env bash
# Makes plain `fastfetch` show the Patrone logo and colors for the current
# user, until the packaged version installs them system-wide. No sudo needed.
# An existing ~/.config/fastfetch/config.jsonc is kept as config.jsonc.before-patrone.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
logo_dir=~/.local/share/patrone/fastfetch
config_dir=~/.config/fastfetch

mkdir -p "$logo_dir" "$config_dir"
cp "$here/logo.txt" "$logo_dir/logo.txt"
if [[ -e $config_dir/config.jsonc ]] && ! grep -q patrone "$config_dir/config.jsonc"; then
    cp "$config_dir/config.jsonc" "$config_dir/config.jsonc.before-patrone"
    echo "Saved the old config as $config_dir/config.jsonc.before-patrone."
fi
# The packaged config points at /usr/share/patrone; point this one at the
# copy in your home directory instead.
sed "s|/usr/share/patrone/fastfetch/logo.txt|$logo_dir/logo.txt|" \
    "$here/config.jsonc" > "$config_dir/config.jsonc"
echo "Installed $config_dir/config.jsonc. Run fastfetch to see it."
