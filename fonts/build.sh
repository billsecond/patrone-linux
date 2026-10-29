#!/usr/bin/env bash
# Builds Patrone Sans from its drawing code (source/), the same bytes every
# time.
#
#   fonts/build.sh             build ttf/, then check it with fontbakery
#   fonts/build.sh --no-check  build ttf/ only
#
# Needs python3 and curl, and the network the first time: the tools go
# into a venv in a cache folder ($PATRONE_SANS_CACHE, by default
# ~/.cache/patrone-sans). No sudo.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
cache=${PATRONE_SANS_CACHE:-$HOME/.cache/patrone-sans}
check=1
[[ ${1:-} == --no-check ]] && check=0

mkdir -p "$cache"
venv=$cache/venv
if [[ ! -x $venv/bin/pip ]]; then
    # Without Ubuntu's python3-venv there's no ensurepip, so pip comes from
    # the Python Packaging Authority's installer instead (no sudo needed)
    python3 -m venv --without-pip "$venv"
    curl -fsSL https://bootstrap.pypa.io/get-pip.py -o "$cache/get-pip.py"
    "$venv/bin/python" "$cache/get-pip.py" --quiet
fi
"$venv/bin/pip" install --quiet -r "$here/requirements.txt"

# The font's dates are the drawing code's last commit's, so a rebuild gives
# the same bytes
SOURCE_DATE_EPOCH=$(git -C "$here" log -1 --format=%ct -- source tools/finish.py 2>/dev/null || date +%s)
export SOURCE_DATE_EPOCH

work=$cache/build
rm -rf "$work"
mkdir -p "$work" "$here/ttf"

run() {
    local log=$1
    shift
    if ! "$@" > "$log" 2>&1; then
        tail -40 "$log" >&2
        echo "Failed: $* (the whole log is $log)" >&2
        exit 1
    fi
}

font="$here/ttf/PatroneSans[wght].ttf"
echo "Patrone Sans: drawing"
run "$work/draw.log" "$venv/bin/python" "$here/source/make_ufos.py" "$work/ufo"
echo "Patrone Sans: compiling"
run "$work/fontmake.log" \
    "$venv/bin/fontmake" -m "$work/ufo/PatroneSans.designspace" -o variable \
    --output-path "$font"
"$venv/bin/python" "$here/tools/finish.py" "$font"

if (( check )); then
    # The report is fontbakery's own text (its markdown writer fails on a
    # font with nothing to warn about)
    echo "Checking with fontbakery (report in fontbakery-report.txt)"
    "$venv/bin/fontbakery" check-universal --succinct --no-colors --loglevel INFO "$font" \
        2> /dev/null | sed "s|$here/||" > "$here/fontbakery-report.txt" || true
    tail -2 "$here/fontbakery-report.txt"
fi

cd "$here/ttf"
sha256sum ./*.ttf
