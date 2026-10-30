#!/usr/bin/env bash
# Checks bash's prompt in a shell set up as a VTE terminal sets one up: what
# the terminal puts in PS0 ends in a backslash, right where the prompt's part
# of PS0 could go. bash only shows PS0 on a terminal, so this needs one.
#
#   tests/prompt.sh              (the package build runs this)
set -u

here=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
failed=0

fail() {
    echo "FAILED: $*"
    failed=1
}

if ! script -qec true /dev/null < /dev/null > /dev/null 2>&1; then
    echo "Prompt: skipped, no pseudo-terminal here"
    exit 0
fi

cat > "$tmp/rc" <<'RC'
printf -v PS0 '\033]666;vte.shell.preexec!\033\'
. "$PATRONE_BASHRC"
RC

# The shell's output for the commands in $1, one a line
run() {
    printf '%b\nexit\n' "$1" |
        PATRONE_BASHRC=$here/bashrc script -qec "bash --noprofile --rcfile $tmp/rc -i" /dev/null
}

output=$(run 'true')
grep -q 'PS1:' <<<"$output" && fail "PS0 printed part of itself as text"
grep -q 'exit code' <<<"$output" && fail "a command that worked showed an exit code"
output=$(run 'false')
grep -q 'exit code 1' <<<"$output" || fail "a failed command showed no exit code"
output=$(run 'false\n\n')
[ "$(grep -c 'exit code' <<<"$output")" = 1 ] || fail "the exit code didn't show exactly once"

[ "$failed" = 0 ] && echo "Prompt: all checks passed"
exit "$failed"
