#!/usr/bin/env bash
# Checks the table of Windows commands and the program that reads it.
#
#   tests/windows-commands.sh              (the package build runs this)
#   tests/windows-commands.sh --installed  (on PatroneOS, with the package
#                                           installed: also checks the system)
set -u

here=$(cd "$(dirname "$0")/.." && pwd)
table=$here/windows-commands.tsv
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
failed=0

fail() {
    echo "FAILED: $*"
    failed=1
}

# Linux commands a Windows user also types, and names Ubuntu's archive has a
# well-known Linux program for. None of them belongs in the table: the real
# one has to run, or Ubuntu's suggestion of its package has to show.
linux_names='ls cat cp mv rm ps kill sleep sort tee pwd echo man mount diff
curl wget ping hostname whoami mkdir rmdir cd find more tree shutdown clear
timeout tar ssh history date time at calc gc gp gm sl ri ac sv gv iex comp
certutil chcp clip replace print pwsh ifconfig netstat route arp nslookup
traceroute'

# The table, a row to a line, without its comments and blank lines
rows() { grep -v -e '^#' -e '^$' "$table"; }

# Its names, in lower case, one to a line
names() { rows | cut -f 1 | tr ' ' '\n' | tr '[:upper:]' '[:lower:]'; }

# Every command it suggests, one to a line: the second column, and what
# follows the = in each option's column
suggestions() {
    rows | awk -F '\t' '{
        print $2
        for (i = 3; i <= NF; i++) {
            if ($i ~ /^[^ =]+=/) print substr($i, index($i, "=") + 1)
        }
    }'
}

# The table's own rules
while IFS= read -r row; do
    case $row in
        *' '$'\t'* | *$'\t'' '* | *' ' | *$'\t'$'\t'* | *$'\t')
            fail "stray space or empty column: $row"
            ;;
    esac
    columns=$(awk -F '\t' '{ print NF }' <<<"$row")
    [ "$columns" -ge 2 ] || fail "needs a name and a Linux command: $row"
    # Each extra is an option, or a sentence
    for ((i = 3; i <= columns; i++)); do
        extra=$(cut -f "$i" <<<"$row")
        [[ $extra =~ ^[^\ =]+=.+$ || $extra == *. ]] || fail "not an option or a sentence: $extra"
    done
done < <(rows)

duplicates=$(names | sort | uniq -d)
[ -z "$duplicates" ] || fail "in the table twice: $duplicates"

for name in $linux_names; do
    if names | grep -qxF -- "$name"; then
        fail "$name is a Linux command, and can't be in the table"
    fi
done

# The program, reading this table, with a stand-in for Ubuntu's
sed -e "s|^table=.*|table=$table|" -e "s|^ubuntu=.*|ubuntu=$tmp/ubuntu|" \
    "$here/command-not-found" >"$tmp/command-not-found"
chmod +x "$tmp/command-not-found"
cat >"$tmp/ubuntu" <<EOF
#!/bin/sh
echo "ubuntu \$*" >>"$tmp/ubuntu-calls"
exit 42
EOF
chmod +x "$tmp/ubuntu"

run() {
    rm -f "$tmp/ubuntu-calls"
    "$tmp/command-not-found" "$@" >"$tmp/out" 2>"$tmp/err"
    status=$?
}

# It says the command is invalid, names the Linux command on stderr, and
# fails, without going on to Ubuntu's program
expect_windows() {
    local name=$1 linux=$2
    run -- "$name"
    [ "$status" = 127 ] || fail "$name: exit status $status"
    [ ! -e "$tmp/ubuntu-calls" ] || fail "$name: went on to Ubuntu's program"
    [ ! -s "$tmp/out" ] || fail "$name: printed on stdout"
    grep -qF "Invalid command: $name" "$tmp/err" || fail "$name: doesn't say it's invalid"
    grep -qxF "    $linux" "$tmp/err" || fail "$name: doesn't suggest $linux"
}

# It goes on to Ubuntu's program, with what it was given
expect_ubuntu() {
    run "$@"
    [ "$status" = 42 ] || fail "$*: exit status $status from Ubuntu's program"
    [ "$(cat "$tmp/ubuntu-calls" 2>/dev/null)" = "ubuntu $*" ] || fail "$*: Ubuntu's program wasn't called with that"
    [ ! -s "$tmp/err" ] || fail "$*: printed something of its own"
}

expect_windows ipconfig 'ip a'
expect_windows IPCONFIG 'ip a'
expect_windows IpConfig.EXE 'ip a'
expect_windows cls clear
expect_windows CLS clear
expect_windows tasklist 'ps aux'
expect_windows findstr 'grep "text" file.txt'
expect_windows Get-ChildItem 'ls -l'
expect_windows get-childitem 'ls -l'
expect_windows GET-PROCESS 'ps aux'
expect_windows gci 'ls -l'
expect_windows 'cd..' 'cd ..'
expect_windows DIR 'ls -l'
expect_windows wsl.exe bash

run -- ipconfig
grep -qF 'ipconfig /all' "$tmp/err" || fail "ipconfig: doesn't say what /all is"

expect_ubuntu -- nosuchcommand
expect_ubuntu -- ipconfi
expect_ubuntu -- xipconfig
expect_ubuntu -- ipconfig.exe.exe
expect_ubuntu -- ip
expect_ubuntu -- 'get'
expect_ubuntu -- '*'
expect_ubuntu -- 'ip*'
expect_ubuntu -- '.*'
expect_ubuntu -- '#'
expect_ubuntu -- 'ipconfig foo'
expect_ubuntu -- ''
expect_ubuntu --help

# It never runs what it suggests
printf 'boom\ttouch %s/ran\n' "$tmp" >"$tmp/table"
sed "s|^table=.*|table=$tmp/table|" "$tmp/command-not-found" >"$tmp/with-table"
chmod +x "$tmp/with-table"
"$tmp/with-table" -- boom 2>/dev/null
[ ! -e "$tmp/ran" ] || fail "it ran what it suggested"

# Without a table it's Ubuntu's program, and without that the shell's words
sed "s|^table=.*|table=$tmp/no-table|" "$tmp/command-not-found" >"$tmp/no-table-run"
chmod +x "$tmp/no-table-run"
"$tmp/no-table-run" -- ipconfig 2>/dev/null
[ $? = 42 ] || fail "with no table it didn't go on to Ubuntu's program"
sed "s|^ubuntu=.*|ubuntu=$tmp/no-ubuntu|" "$tmp/command-not-found" >"$tmp/no-ubuntu-run"
chmod +x "$tmp/no-ubuntu-run"
message=$("$tmp/no-ubuntu-run" -- nosuchcommand 2>&1)
[ $? = 127 ] || fail "with no Ubuntu program, the exit status isn't 127"
[ "$message" = 'nosuchcommand: command not found' ] || fail "with no Ubuntu program it said: $message"

# Every message fits a terminal of 80 columns, and has no color in a file
for name in $(names); do
    run -- "$name"
    [ "$status" = 127 ] || fail "$name: exit status $status"
    width=$(awk '{ if (length > w) w = length } END { print w + 0 }' "$tmp/err")
    [ "$width" -le 80 ] || fail "$name: a line of $width columns"
    ! grep -q $'\033' "$tmp/err" || fail "$name: color in a file"
done

if [ "${1:-}" = --installed ]; then
    [ "$(dpkg-divert --listpackage /usr/lib/command-not-found)" = patrone-shell-config ] ||
        fail "/usr/lib/command-not-found isn't diverted"
    [ -x /usr/lib/command-not-found.ubuntu ] || fail "Ubuntu's program isn't beside it"
    cmp -s "$here/command-not-found" /usr/lib/command-not-found || fail "another version is installed"
    cmp -s "$table" /usr/share/patrone/windows-commands.tsv || fail "another table is installed"

    # What it suggests is here to use
    while read -r command rest; do
        if [ "$command" = sudo ]; then read -r command rest <<<"$rest"; fi
        type -t -- "$command" >/dev/null || fail "suggests $command, which isn't here"
    done < <(suggestions)

    # Nothing in the table runs as a real command, but for the ones that only
    # get here spelled in capitals, or from one of the two shells
    for name in $(names); do
        case $name in dir | type | set | help | where | chdir) continue ;; esac
        if type -t -- "$name" >/dev/null || zsh -fc "whence -- '$name'" >/dev/null 2>&1; then
            fail "$name is a real command here"
        fi
    done
fi

if [ "$failed" = 0 ]; then
    echo "Windows commands: all checks passed ($(names | wc -l) names)"
fi
exit "$failed"
