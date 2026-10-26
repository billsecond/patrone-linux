#!/bin/bash
# Runs patrone-remote-desktop's tests (make check). Needs no root and no real
# password: the plugin talks to a stand-in helper, and the helper is only
# given sign-ins it must refuse.
#
#   tests/run.sh PLUGIN.so TEST-PLUGIN AUTH-HELPER
set -euo pipefail

plugin=$1
test_plugin=$2
auth=$3
here=$(dirname "$0")
socket=/tmp/patrone-rd-test.sock
failures=0

python3 "$here/fake-auth.py" "$socket" &
helper=$!
trap 'kill $helper 2> /dev/null || true; rm -f "$socket"' EXIT
for _ in $(seq 50); do
    [[ -S $socket ]] && break
    sleep 0.1
done

"$test_plugin" "$plugin" || failures=$((failures + 1))
kill $helper
wait $helper 2> /dev/null || true
rm -f "$socket"
"$test_plugin" "$plugin" unreachable || failures=$((failures + 1))

# The sign-in helper must refuse these before or at PAM
helper_refuses() {
    local test=$1 answer
    answer=$(printf '%b' "$2" | "$auth" 2> /dev/null || true)
    if [[ $answer == denied ]]; then
        echo "ok   helper: $test"
    else
        echo "FAIL helper: $test (answered \"$answer\")"
        failures=$((failures + 1))
    fi
}
helper_refuses "malformed request" 'alice'
helper_refuses "unusual account name" '../etc\0secret\0192.0.2.7\0'
helper_refuses "account name too long" "$(printf 'a%.0s' $(seq 40))\\0secret\\0192.0.2.7\\0"
helper_refuses "unknown account" 'nosuch-patrone-user\0secret\0192.0.2.7\0'

# --console takes the account from the socket, so without one it must refuse.
answer=$("$auth" --console < /dev/null 2> /dev/null || true)
if [[ $answer == denied ]]; then
    echo "ok   helper: console request without a socket"
else
    echo "FAIL helper: console request without a socket (answered \"$answer\")"
    failures=$((failures + 1))
fi

# The proxy must be able to read the shipped config (it once wasn't: FreeRDP
# rejects "#" comments). It stops at the certificates, which don't exist here.
config_parses() {
    local log
    log=$(timeout 5 freerdp-proxy "$here/../data/proxy.ini" 2>&1 || true)
    if [[ $log == *"failed to parse"* ]]; then
        echo "FAIL proxy: reads data/proxy.ini"
        failures=$((failures + 1))
    else
        echo "ok   proxy: reads data/proxy.ini"
    fi
}
if command -v freerdp-proxy > /dev/null; then
    config_parses
else
    echo "skip proxy: freerdp-proxy isn't installed"
fi

if ((failures)); then
    echo "$failures test run(s) failed"
    exit 1
fi
echo "All tests passed"
