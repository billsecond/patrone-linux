#!/usr/bin/env bash
# Builds a Patrone .deb on the dev VM, from this checkout.
#
#   ./packaging/build.sh patrone-terminal [dpkg-buildpackage options]
#   ./packaging/build.sh patrone-shell-config
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
    *)
        echo "Unknown package: $package" >&2
        exit 1
        ;;
esac

debian="$root/packaging/$package/debian"
version=$(dpkg-parsechangelog -l "$debian/changelog" -S Version)
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
