#!/usr/bin/env bash
# Builds Patrone Sans from Noto Sans's own sources, the same bytes every time.
#
#   fonts/build.sh             build ttf/, then check it with fontbakery
#   fonts/build.sh --no-check  build ttf/ only
#
# Needs python3, git and curl, and the network the first time: the tools go
# into a venv and Noto Sans's sources into a cache folder
# ($PATRONE_SANS_CACHE, by default ~/.cache/patrone-sans). No sudo.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
cache=${PATRONE_SANS_CACHE:-$HOME/.cache/patrone-sans}
upstream_url=https://github.com/notofonts/latin-greek-cyrillic
upstream_tag=NotoSans-v2.015
upstream_commit=c4a321e123e4d4ff315f57f4e0adf294fe3a95be
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

upstream=$cache/upstream
if [[ ! -d $upstream/.git ]]; then
    git clone --quiet --depth 1 --branch "$upstream_tag" "$upstream_url" "$upstream"
fi
if [[ $(git -C "$upstream" rev-parse HEAD) != "$upstream_commit" ]]; then
    echo "$upstream isn't Noto Sans $upstream_tag ($upstream_commit)" >&2
    exit 1
fi

# The fonts' dates are the sources' date, so a rebuild gives the same bytes
SOURCE_DATE_EPOCH=$(git -C "$upstream" log -1 --format=%ct)
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

for style in NotoSans NotoSans-Italic; do
    patrone=${style/NotoSans/PatroneSans}
    echo "$patrone: sources"
    run "$work/$style-glyphs2ufo.log" \
        "$venv/bin/glyphs2ufo" -m "$work/$style" \
        --generate-GDEF --write-public-skip-export-glyphs \
        "$upstream/sources/$style.glyphspackage"
    "$venv/bin/python" "$here/tools/patrone_sans.py" "$work/$style/$style.designspace"
    echo "$patrone: compiling"
    run "$work/$style-fontmake.log" \
        "$venv/bin/fontmake" -m "$work/$style/$patrone.designspace" \
        -o variable --flatten-components \
        --filter DecomposeTransformedComponentsFilter \
        --output-path "$here/ttf/$patrone[wght].ttf"
done

roman="$here/ttf/PatroneSans[wght].ttf"
italic="$here/ttf/PatroneSans-Italic[wght].ttf"
run "$work/gen-stat.log" "$venv/bin/gftools" gen-stat --inplace "$roman" "$italic"
"$venv/bin/python" "$here/tools/finish.py" "$roman" "$italic"

if (( check )); then
    echo "Checking with fontbakery (report in $work/fontbakery.md)"
    "$venv/bin/fontbakery" check-universal --succinct --loglevel WARN \
        --ghmarkdown "$work/fontbakery.md" "$roman" "$italic" || true
fi

cd "$here/ttf"
sha256sum ./*.ttf
