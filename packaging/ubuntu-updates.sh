#!/usr/bin/env bash
# Keeps Patrone's builds of Ubuntu's packages up with Ubuntu. When Ubuntu
# publishes a newer GDM, GNOME Settings or first-login setup for resolute (in
# its release, security or updates pocket), builds it again with Patrone's
# changes (gdm/build.sh, gnome-settings/build.sh, gnome-initial-setup/build.sh)
# and publishes it (publish.sh): installs keep Patrone's build until then
# (desktop/apt/patrone.pref). Also says when the installer snap's channel
# has moved past the revision installer/build.sh follows. That one isn't
# rebuilt here: the installer has to match the snap on the ISO Patrone
# remasters, so it's rebuilt with the next ISO.
#
#   ./packaging/ubuntu-updates.sh            (on the dev VM)
#   ./packaging/ubuntu-updates.sh --check    (only say what's behind)
#
# Runs daily on the dev VM (scripts/vm/patrone-ubuntu-updates.timer). It
# builds from a clean checkout of main in WORK (default
# ~/.cache/patrone-ubuntu-updates), never from this one, which may be on a
# branch: this checkout's origin, fetched each time. Whatever it can't do (a
# build that fails, the installer) becomes an issue in billsecond/patrone-apt;
# that repository's own daily check opens one too while a build is behind.
#
# First, in the same checkout, it runs main-updates.sh, which publishes what's
# merged into main and not yet published. Ubuntu's updates come next whatever
# happens there, and the run ends with an error if that part had one.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/.cache/patrone-ubuntu-updates}
src=$work/patrone-linux
repo=${APT_REPO:-$HOME/src/patrone-apt}
github=${APT_GITHUB:-billsecond/patrone-apt}
check_only=
[ "${1:-}" != --check ] || check_only=1

mkdir -p "$work/logs"
exec 9>"$work/lock"
flock -n 9 || { echo "Already running" >&2; exit 0; }

# Run from main's copy of this script, in the clean checkout
if [ "$here" != "$src/packaging" ]; then
    url=$(git -C "$here/.." remote get-url origin)
    [ -d "$src/.git" ] || git clone -q "$url" "$src"
    git -C "$src" remote set-url origin "$url"
    git -C "$src" fetch -q origin main
    git -C "$src" checkout -q --force --detach origin/main
    exec 9>&-
    exec "$src/packaging/ubuntu-updates.sh" "$@"
fi
root=$src

# An issue in the apt repository, unless one with that title is open
flag() {
    local title=$1 body=$2
    if [ -n "$(gh issue list -R "$github" --state open --search "\"$title\" in:title" \
        --json title --jq ".[] | select(.title == \"$title\") | .title")" ]; then
        return
    fi
    gh issue create -R "$github" --title "$title" --body "$body" >/dev/null
    echo "Opened an issue: $title"
}

[ -d "$repo/.git" ] || git clone -q "https://github.com/$github" "$repo"
git -C "$repo" pull -q --ff-only

# What's merged into main and not yet published
status=0
LOGS=$work/logs APT_REPO=$repo APT_GITHUB=$github \
    "$root/packaging/main-updates.sh" ${check_only:+--check} || status=$?

# Patrone's builds of Ubuntu's packages, and how each is built
declare -A build=(
    [gdm3]=gdm/build.sh
    [gnome-control-center]=gnome-settings/build.sh
    [gnome-initial-setup]=gnome-initial-setup/build.sh
)
"$repo/tools/check-ubuntu.sh" >"$work/check"
while read -r state source built_from newest pocket; do
    echo "$source: Patrone's build is from Ubuntu's $built_from; Ubuntu's newest is $newest ($pocket)"
    [ "$state" = behind ] && [ -z "$check_only" ] || continue
    log=$work/logs/$source-${newest#*:}.log
    if [ -z "${build[$source]:-}" ]; then
        flag "Rebuilding $source $newest: no build script" \
            "Ubuntu has published $source $newest, and Patrone's index has a build of $source, but packaging/ubuntu-updates.sh doesn't know how to build it. Add it to the list there."
        continue
    fi
    echo "Building $source $newest with Patrone's changes (log: $log)"
    if ! nice -n 19 "$root/${build[$source]}" "$newest" >"$log" 2>&1; then
        flag "Rebuilding $source $newest failed" \
            "packaging/ubuntu-updates.sh on the dev VM couldn't build Ubuntu's $source $newest with Patrone's changes (${build[$source]}). The log is $log on the VM; its end:

\`\`\`
$(tail -n 40 "$log")
\`\`\`

Patrone Linux installs keep Patrone's build from $built_from until this is fixed and published. If Patrone's patches no longer apply, update them in Patrone Linux's repository; if the build needs new packages, install Ubuntu's build dependencies on the VM."
        continue
    fi
    # The build's binary packages, published together
    "$root/packaging/publish.sh" "$source" >>"$log" 2>&1 || {
        flag "Publishing $source $newest failed" \
            "The build of $source $newest worked, but packaging/publish.sh didn't. The log is $log on the dev VM; its end:

\`\`\`
$(tail -n 20 "$log")
\`\`\`"
        continue
    }
    echo "Published $source from Ubuntu's $newest"
done <"$work/check"

# The installer's snap: a new revision in the channel installer/build.sh
# follows means the next ISO will bring a new installer engine
eval "$(grep -E '^snap_(channel|revision)=' "$root/installer/build.sh")"
latest=$(curl -fsS --retry 3 -H 'Snap-Device-Series: 16' \
    'https://api.snapcraft.io/v2/snaps/info/ubuntu-desktop-bootstrap?fields=revision,version' |
    jq -r --arg c "$snap_channel" '."channel-map"[]
        | select(.channel.architecture == "amd64" and (.channel.track + "/" + .channel.risk) == $c)
        | "\(.revision) \(.version)"')
read -r revision version <<<"$latest"
echo "Installer snap: $snap_channel is revision ${revision:-?} ($version); Patrone's installer follows $snap_revision"
if [ -n "$revision" ] && [ "$revision" != "$snap_revision" ] && [ -z "$check_only" ]; then
    flag "The installer snap has moved: ubuntu-desktop-bootstrap $snap_channel is revision $revision" \
        "Ubuntu's installer snap, ubuntu-desktop-bootstrap, is at revision $revision ($version) in $snap_channel. Patrone's installer front end (installer/build.sh) follows revision $snap_revision, the one on Ubuntu 26.04.1's ISO, which Patrone's ISO remasters, so nothing changes until Patrone builds an ISO from a newer Ubuntu ISO.

Then: take provision_commit from that snap's snap/manifest.yaml, the Flutter version from .tool-versions and ubuntu_flavor's from apps/ubuntu_bootstrap/pubspec.lock, set snap_revision, and build the installer again (installer/README.md)."
fi
exit "$status"
