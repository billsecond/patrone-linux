#!/bin/bash
# Builds Patrone's installer: Ubuntu's installer front end (the Flutter app
# ubuntu_bootstrap, from canonical/ubuntu-desktop-provision), at the commit
# the ISO's installer snap was built from, with "Patrone Linux" as its
# flavor's name and in its own text, where Ubuntu's says "Ubuntu".
# Everything else, the install engine included, stays the snap's
# (installer/README.md).
#
#   installer/build.sh      (on the dev VM, as yourself)
#
# The first run fetches the installer's source, Flutter (the version the
# installer pins) and ubuntu_flavor (from pub.dev) into WORK (default
# ~/.cache/patrone-installer). The app lands in installer/bundle, for
# packaging/build.sh patrone-installer.
set -euo pipefail

# ubuntu-desktop-bootstrap 26.04.1, snap revision 665, on Ubuntu 26.04.1's
# ISO: its snap/manifest.yaml names the commit, .tool-versions there the
# Flutter version, and apps/ubuntu_bootstrap/pubspec.lock ubuntu_flavor's.
provision_commit=6623407beba8f4d8cb0f331a232d0a7d271a9f7a
flutter_version=3.44.1
flavor_version=0.5.0+1

here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/.cache/patrone-installer}
src=$work/ubuntu-desktop-provision
flavor=$work/ubuntu_flavor
mkdir -p "$work"

if [ ! -d "$src" ]; then
    git clone --filter=blob:none https://github.com/canonical/ubuntu-desktop-provision.git "$src"
fi
# Undoes the last build's changes to the source, too
git -C "$src" checkout -q --force "$provision_commit"

if [ ! -x "$work/flutter/bin/flutter" ]; then
    git clone -b "$flutter_version" --depth 1 https://github.com/flutter/flutter.git "$work/flutter"
fi
export PATH="$work/flutter/bin:$PATH"
flutter config --no-analytics >/dev/null
dart --disable-analytics >/dev/null

# Ubuntu's flavor, named Patrone Linux: the installer shows the flavor's
# name wherever its text says {DISTRO} ("Welcome to {DISTRO}", "Install
# {DISTRO}"), and knows only Ubuntu's flavors (flavor_service.dart).
rm -rf "$flavor"
mkdir -p "$flavor"
curl -fsSL "https://pub.dev/api/archives/ubuntu_flavor-$flavor_version.tar.gz" | tar -xz -C "$flavor"
sed -i "s/^  ubuntu('Ubuntu'),\$/  ubuntu('Patrone Linux'),/" "$flavor/lib/src/ubuntu_flavor.dart"
grep -q "^  ubuntu('Patrone Linux'),\$" "$flavor/lib/src/ubuntu_flavor.dart"

cd "$src"
# Ubuntu's own lock files, as the snap's build uses them
dart pub get
dart run melos bootstrap

# The text that names Ubuntu itself in place of {DISTRO}
python3 -I "$here/debrand_l10n.py" apps/ubuntu_bootstrap/lib/l10n packages/ubuntu_provision/lib/src/l10n

cd apps/ubuntu_bootstrap
printf '  ubuntu_flavor:\n    path: %s\n' "$flavor" >>pubspec_overrides.yaml
flutter pub get
flutter build linux --release

rm -rf "$here/bundle"
cp -r build/linux/x64/release/bundle "$here/bundle"
echo "Built $here/bundle"
