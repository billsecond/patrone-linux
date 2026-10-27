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

# --screen-sign-in acts only as root, for the login screen's services: here
# (not root) it must do nothing, quietly, and never fail the sign-in.
answer=$(PAM_SERVICE=gdm-password PAM_USER=$(id -un) "$auth" --screen-sign-in 2>&1)
if [[ $? == 0 && -z $answer ]]; then
    echo "ok   helper: screen sign-in does nothing without root"
else
    echo "FAIL helper: screen sign-in without root (answered \"$answer\")"
    failures=$((failures + 1))
fi

# --console takes the account from the socket, so without one it must refuse.
answer=$("$auth" --console < /dev/null 2> /dev/null || true)
if [[ $answer == denied ]]; then
    echo "ok   helper: console request without a socket"
else
    echo "FAIL helper: console request without a socket (answered \"$answer\")"
    failures=$((failures + 1))
fi

# --confirm (Settings' password check) acts only as root: here it must refuse
# without asking PAM.
answer=$(printf secret | "$auth" --confirm "$(id -un)" 2> /dev/null || true)
if [[ $answer == denied ]]; then
    echo "ok   helper: Settings' password check refuses without root"
else
    echo "FAIL helper: Settings' password check without root (answered \"$answer\")"
    failures=$((failures + 1))
fi

# Settings' helper runs only through pkexec, and every polkit action for it
# names it and one of its commands.
if ! "$here/../libexec/settings" status > /dev/null 2>&1; then
    echo "ok   settings: refuses without pkexec"
else
    echo "FAIL settings: ran without pkexec"
    failures=$((failures + 1))
fi
if python3 - "$here/../data/org.patrone.remote-desktop.policy" << 'EOF'; then
import sys
import xml.etree.ElementTree as tree

helper = "/usr/libexec/patrone-remote-desktop/settings"
commands = set()
for action in tree.parse(sys.argv[1]).getroot().iter("action"):
    notes = {a.get("key"): a.text for a in action.iter("annotate")}
    assert notes["org.freedesktop.policykit.exec.path"] == helper, action.get("id")
    commands.add(notes["org.freedesktop.policykit.exec.argv1"])
    defaults = action.find("defaults")
    # Patrone's desktops have no seat: "any" must allow what "active" does
    assert len({d.text for d in defaults}) == 1, action.get("id")
assert commands == {"status", "enable", "disable", "password"}, commands
EOF
    echo "ok   settings: polkit actions"
else
    echo "FAIL settings: polkit actions"
    failures=$((failures + 1))
fi

# --remember keeps an account's NT hash in the SAM file NLA checks, read back
# here as FreeRDP's NTLM reads it. A test build: it writes a file in /tmp, and
# needs neither root nor the remote-desktop group.
expect() {
    local test=$1
    shift
    if "$@"; then
        echo "ok   remember: $test"
    else
        echo "FAIL remember: $test"
        failures=$((failures + 1))
    fi
}
remember=$(dirname "$test_plugin")/auth-remember-test
read_sam=$(dirname "$test_plugin")/test-sam
sam=/tmp/patrone-rd-test.sam
me=$(id -un)
if (($(id -u) >= 1000)); then
    printf 'someone:::00000000000000000000000000000000:\n' > "$sam"
    printf '%s' password | PAM_USER=$me "$remember" --remember
    expect "the NT hash of \"password\"" \
        "$read_sam" "$sam" "$me" "" 8846F7EAEE8FB117AD06BDD830B7586C
    expect "found when the client sends a domain" \
        "$read_sam" "$sam" "$me" SOME-PC 8846F7EAEE8FB117AD06BDD830B7586C
    printf '%s' Password1 | PAM_USER=$me "$remember" --remember
    expect "a new password replaces it" \
        "$read_sam" "$sam" "$me" "" 64F12CDDAA88057E06A81B54E73B949B
    expect "one entry per account" test "$(grep -c "^$me:" "$sam")" = 1
    expect "other accounts' entries kept" \
        "$read_sam" "$sam" someone "" 00000000000000000000000000000000
    expect "readable by its owner only (no proxy group here)" \
        test "$(stat -c %a "$sam")" = 600
    rm -f "$sam" "$sam.lock"
else
    echo "skip remember: needs an account with UID 1000 or above"
fi

# The login-screen extension, under plain gjs with GNOME Shell's modules
# stood in for
if command -v gjs > /dev/null; then
    extension_test=$(mktemp --suffix=.js)
    {
        cat "$here/login-screen-stubs.js"
        sed -e '/^import /d' -e 's/^export default class/class/' \
            "$here/../login-screen/patrone-login@patrone-linux/extension.js"
        cat "$here/login-screen-test.js"
    } > "$extension_test"
    gjs -m "$extension_test" 2>&1 | grep -v "^Gjs-Message" || failures=$((failures + 1))
    rm -f "$extension_test"

    # The polkit rule for Patrone's desktops, with polkit stood in for
    gjs -m "$here/polkit-rule-test.js" "$here/../data/patrone-desktops.rules" ||
        failures=$((failures + 1))
else
    echo "skip login screen and polkit rule: gjs isn't installed"
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
