#!/usr/bin/env bash
# Writes the licence notices of the Rust crates in a package's vendor folder
# (made by cargo vendor) to standard output. build.sh installs the result as
# /usr/share/doc/PACKAGE/third-party-licenses.txt.gz.
#
#   packaging/third-party-licenses.sh VENDOR > third-party-licenses.txt
#
# The crates are compiled into the package's program, and their licences
# (MIT, BSD, Apache-2.0, ISC, Zlib, Unicode and so on) ask for their notices
# to go with every copy of it. Each crate's licence, copyright and notice
# files are copied as they are. Every crate cargo vendored is listed, so a
# few that only help build the program, or are only used on other systems,
# are listed too.
set -euo pipefail

vendor=${1:?Usage: $0 VENDOR}

# field FILE KEY: KEY's value in the [package] table of a vendored
# Cargo.toml, which cargo has normalized to one `key = "value"` per line
field() {
    awk -v key="$2" '
        /^\[/ { in_package = ($0 == "[package]") }
        in_package && $1 == key && $2 == "=" {
            sub(/^[^=]*= *"/, ""); sub(/" *$/, ""); print; exit
        }' "$1"
}

echo "Third-party Rust code in this package"
echo
echo "This program is built with the Rust crates below. Each one's licence"
echo "and copyright notices follow, as the crate ships them."

for dir in "$vendor"/*/; do
    toml=$dir/Cargo.toml
    [[ -f $toml ]] || continue
    name=$(field "$toml" name)
    version=$(field "$toml" version)
    license=$(field "$toml" license)
    license_file=$(field "$toml" license-file)

    echo
    echo "================================================================"
    echo "$name $version"
    echo "License: ${license:-see the file below}"
    echo "================================================================"

    files=()
    while IFS= read -r -d '' file; do
        files+=("$file")
    done < <(find "$dir" -maxdepth 1 -type f \( -iname 'LICEN[CS]E*' \
        -o -iname 'COPYING*' -o -iname 'COPYRIGHT*' -o -iname 'NOTICE*' \
        -o -iname 'UNLICENSE*' \) -print0 | sort -z)
    if [[ -n $license_file && -f $dir/$license_file ]] &&
        [[ " ${files[*]} " != *" $dir$license_file "* ]]; then
        files+=("$dir$license_file")
    fi

    if [[ ${#files[@]} == 0 ]]; then
        echo
        echo "(The crate has no licence file of its own; its Cargo.toml"
        echo "names the licence above.)"
    fi
    for file in "${files[@]}"; do
        echo
        echo "--- ${file#"$dir"}"
        echo
        cat "$file"
    done
done
