#!/usr/bin/env bash
# Builds a Patrone .deb on the dev VM, from this checkout.
#
#   ./packaging/build.sh patrone-terminal [dpkg-buildpackage options]
#   ./packaging/build.sh patrone-shell-config
#   ./packaging/build.sh patrone-branding
#   ./packaging/build.sh patrone-default-apps
#   ./packaging/build.sh patrone-remote-desktop
#   ./packaging/build.sh patrone-file-manager -d
#   ./packaging/build.sh patrone-gnome-taskbar
#
# The packages land in out/. For a Rust package the crates are vendored into
# the source tree first, so the build itself runs offline, as it will in a
# clean sbuild chroot. Pass -d to skip the Build-Depends check, for example
# when Rust comes from rustup instead of Ubuntu's rustc and cargo.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
package=${1:?Usage: $0 PACKAGE [dpkg-buildpackage options]}
shift

case $package in
    patrone-terminal) source_dir=terminal ;;
    patrone-shell-config) source_dir=shell-config ;;
    patrone-branding) source_dir=branding ;;
    patrone-default-apps) source_dir=default-apps ;;
    patrone-remote-desktop) source_dir=remote-desktop ;;
    patrone-file-manager) source_dir=file-manager ;;
    patrone-gnome-taskbar) source_dir=shell/gnome ;;
    *)
        echo "Unknown package: $package" >&2
        exit 1
        ;;
esac

debian="$root/packaging/$package/debian"
version=$(dpkg-parsechangelog -l "$debian/changelog" -S Version)

# debhelper replaces its placeholder wherever it appears in a maintainer
# script, comments included, so it must stand on a line of its own.
for script in "$debian"/{preinst,postinst,prerm,postrm}; do
    [[ -f $script ]] || continue
    if grep -n '#DEBHELPER#' "$script" | grep -v '^[0-9]*:#DEBHELPER#$'; then
        echo "$script: put #DEBHELPER# only on a line of its own (above)" >&2
        exit 1
    fi
done
build="$root/out/build"
work="$build/$package-$version"

rm -rf "$work"
mkdir -p "$work"
tar -C "$root/$source_dir" --exclude=./target -cf - . | tar -C "$work" -xf -
cp -r "$debian" "$work/"

if [[ -f $work/Cargo.toml ]]; then
    (cd "$work" && cargo vendor --locked --quiet vendor)
    mkdir -p "$work/.cargo"
    cat > "$work/.cargo/config.toml" << 'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF
fi

(cd "$work" && dpkg-buildpackage -us -uc -b "$@")

mkdir -p "$root/out"
mv "$build"/"$package"*_"$version"_*.deb "$root/out/"
ls -l "$root/out/$package"*_"$version"_*.deb
