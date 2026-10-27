#!/bin/bash
# Runs the taskbar in a throwaway GNOME Shell with no screen, for trying it
# and taking screenshots without touching anyone's desktop. It gets its own
# D-Bus session, settings, runtime folder and Wayland display, all under
# /tmp/patrone-taskbar-test, and runs in Ubuntu's session mode (Yaru, its
# extensions) like the real desktop.
#
#   tests/headless.sh start [WIDTHxHEIGHT] [light|dark]
#   tests/headless.sh eval 'JAVASCRIPT'      run in the shell, print the result
#   tests/headless.sh shot FILE.png          screenshot
#   tests/headless.sh key KEY                press and release a key (Return, Escape, Down...)
#   tests/headless.sh super                  press and release the Super key
#   tests/headless.sh type TEXT              type text
#   tests/headless.sh click X Y [BUTTON]     click (1 left, 2 middle, 3 right)
#   tests/headless.sh drag X1 Y1 X2 Y2       press, hold, move and let go
#   tests/headless.sh move X Y               move the pointer
#   tests/headless.sh run COMMAND...         start an app on its display
#   tests/headless.sh close-apps             end the apps it started
#   tests/headless.sh log                    the shell's output
#   tests/headless.sh stop
#
# A helper extension (tests/unsafe@patrone-linux) turns on GNOME Shell's
# "unsafe mode" there, which is what lets eval and shot work.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
extension=$here/../patrone-taskbar@patrone-linux
uuid=patrone-taskbar@patrone-linux
base=/tmp/patrone-taskbar-test
display=wayland-patrone-test

environment() {
    export XDG_CONFIG_HOME=$base/config
    export XDG_DATA_HOME=$base/data
    export XDG_CACHE_HOME=$base/cache
    export XDG_RUNTIME_DIR=$base/run
    # As in the desktop session: snaps' apps (Chromium) are in snapd's folder
    export XDG_DATA_DIRS=/usr/share/ubuntu:/usr/share/gnome:/usr/local/share/:/usr/share/:/var/lib/snapd/desktop
    export XDG_CURRENT_DESKTOP=ubuntu:GNOME
    export XDG_SESSION_DESKTOP=ubuntu
    export XDG_SESSION_TYPE=wayland
    export GNOME_SHELL_SESSION_MODE=ubuntu
    unset DISPLAY WAYLAND_DISPLAY
    if [[ -f $base/bus ]]; then
        DBUS_SESSION_BUS_ADDRESS=$(cat "$base/bus")
        export DBUS_SESSION_BUS_ADDRESS
    fi
}

install_extensions() {
    local extensions=$XDG_DATA_HOME/gnome-shell/extensions
    rm -rf "${extensions:?}/$uuid" "$extensions/unsafe@patrone-linux"
    mkdir -p "$extensions"
    cp -r "$extension" "$extensions/$uuid"
    glib-compile-schemas "$extensions/$uuid/schemas"
    cp -r "$here/unsafe@patrone-linux" "$extensions/"

    # The taskbar's settings and Patrone Settings' theme setting, where
    # other programs (Patrone Settings) find them too
    local schemas=$XDG_DATA_HOME/glib-2.0/schemas
    mkdir -p "$schemas"
    cp "$extension"/schemas/*.gschema.xml "$schemas/"
    if [[ -f $here/../../../settings/data/org.patrone.desktop.gschema.xml ]]; then
        cp "$here/../../../settings/data/org.patrone.desktop.gschema.xml" "$schemas/"
    fi
    glib-compile-schemas "$schemas"
}

# A few files in the recent files list, as if opened in GTK apps, for the
# Start menu's "Recommended" and the taskbar's jump lists
sample_files() {
    local files=$base/files now
    mkdir -p "$files"
    now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    {
        echo '<?xml version="1.0" encoding="UTF-8"?>'
        echo '<xbel version="1.0" xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks" xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info">'
        local name mime app exec
        while IFS='|' read -r name mime app exec; do
            echo "sample $name" > "$files/$name"
            cat << EOF
  <bookmark href="file://$files/$name" added="$now" modified="$now" visited="$now">
    <info><metadata owner="http://freedesktop.org">
      <mime:mime-type type="$mime"/>
      <bookmark:applications><bookmark:application name="$app" exec="&apos;$exec %u&apos;" modified="$now" count="1"/></bookmark:applications>
    </metadata></info>
  </bookmark>
EOF
        done << 'EOF'
Quarterly report.txt|text/plain|org.gnome.TextEditor|gnome-text-editor
Meeting notes.txt|text/plain|org.gnome.TextEditor|gnome-text-editor
Budget 2027.ods|application/vnd.oasis.opendocument.spreadsheet|LibreOffice|libreoffice
Team photo.png|image/png|Image Viewer|loupe
Welcome letter.odt|application/vnd.oasis.opendocument.text|LibreOffice|libreoffice
EOF
        echo '</xbel>'
    } > "$XDG_DATA_HOME/recently-used.xbel"
}

shell_eval() {
    gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.gnome.Shell.Eval "$1"
}

# Virtual input devices in the shell, made once and kept on globalThis
input_js() {
    cat << 'EOF'
const Clutter = imports.gi.Clutter;
const GLib = imports.gi.GLib;
const seat = global.stage.get_context().get_backend().get_default_seat();
globalThis.testKeyboard ??= seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
globalThis.testPointer ??= seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
const now = () => GLib.get_monotonic_time();
const tap = keyval => {
    testKeyboard.notify_keyval(now(), keyval, Clutter.KeyState.PRESSED);
    testKeyboard.notify_keyval(now(), keyval, Clutter.KeyState.RELEASED);
};
EOF
}

case ${1:-} in
start)
    size=${2:-1600x900}
    style=${3:-light}
    "$0" stop > /dev/null 2>&1 || true
    rm -rf "$base"
    mkdir -p "$base"/{config,data,cache,run}
    chmod 700 "$base/run"
    environment
    install_extensions
    sample_files

    dbus-daemon --session --fork --print-address=3 --print-pid=4 \
        3> "$base/bus" 4> "$base/bus.pid"
    environment
    gsettings set org.gnome.shell enabled-extensions "['$uuid', 'unsafe@patrone-linux']"
    # Not Ubuntu's desktop icons, which end every desktop-icons process of
    # the account when they start, the real desktop's included; nor its
    # snap permission prompts, which deal with the system's snapd. Even a
    # copy run from a folder of its own (Ubuntu's session loads it only
    # from system folders, so it took a data folder of the test's own,
    # with the system's copy hidden by bwrap) restarted the real desktop's
    # desktop icons when it started and when it stopped (2026-10-26), so
    # don't run them here at all.
    gsettings set org.gnome.shell disabled-extensions \
        "['ubuntu-dock@ubuntu.com', 'ding@rastersoft.com', 'snapd-prompting@canonical.com']"
    gsettings set org.gnome.shell welcome-dialog-last-shown-version '999'
    gsettings set org.gnome.desktop.interface color-scheme "prefer-$style"

    nohup gnome-shell --headless --wayland --no-x11 --mode=ubuntu \
        --virtual-monitor "$size" --wayland-display "$display" \
        > "$base/shell.log" 2>&1 &
    echo $! > "$base/shell.pid"

    for _ in $(seq 100); do
        if shell_eval 'global.context.unsafe_mode' 2> /dev/null | grep -q true; then
            echo "GNOME Shell is running ($size, $style)"
            exit 0
        fi
        sleep 0.2
    done
    echo "GNOME Shell didn't start; see $base/shell.log" >&2
    exit 1
    ;;
eval)
    environment
    shell_eval "$2"
    ;;
shot)
    environment
    file=$(realpath -m "$2")
    gdbus call --session --dest org.gnome.Shell.Screenshot \
        --object-path /org/gnome/Shell/Screenshot \
        --method org.gnome.Shell.Screenshot.Screenshot false false "$file" > /dev/null
    echo "$file"
    ;;
key)
    environment
    shell_eval "$(input_js) tap(Clutter.KEY_$2); 'ok'" > /dev/null
    ;;
super)
    # The Super key by its key code (evdev KEY_LEFTMETA), as a keyboard
    # sends it, so GNOME sees an overlay-key press
    environment
    shell_eval "$(input_js)
        testKeyboard.notify_key(now(), 125, Clutter.KeyState.PRESSED);
        testKeyboard.notify_key(now(), 125, Clutter.KeyState.RELEASED);
        'ok'" > /dev/null
    ;;
type)
    environment
    text=$(printf '%s' "$2" | sed 's/\\/\\\\/g; s/"/\\"/g')
    shell_eval "$(input_js) for (const c of \"$text\") tap(Clutter.unicode_to_keysym(c.codePointAt(0))); 'ok'" > /dev/null
    ;;
move)
    environment
    shell_eval "$(input_js) testPointer.notify_absolute_motion(now(), $2, $3); 'ok'" > /dev/null
    ;;
click)
    # The pointer arrives first, then presses: apps (not the shell) miss a
    # press that comes with the pointer's arrival
    environment
    shell_eval "$(input_js)
        testPointer.notify_absolute_motion(now(), $2, $3);
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 150, () => {
            testPointer.notify_button(now(), ${4:-1}, Clutter.ButtonState.PRESSED);
            testPointer.notify_button(now(), ${4:-1}, Clutter.ButtonState.RELEASED);
            return GLib.SOURCE_REMOVE;
        });
        'ok'" > /dev/null
    sleep 0.3
    ;;
drag)
    # Press, hold past the drag delay, move there in steps, let go: timed
    # in the shell, so it returns before the drag is over (about 1.2s)
    environment
    shell_eval "$(input_js)
        testPointer.notify_absolute_motion(now(), $2, $3);
        testPointer.notify_button(now(), 1, Clutter.ButtonState.PRESSED);
        let step = 0;
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 30, () => {
            step++;
            if (step < 10)
                return GLib.SOURCE_CONTINUE;
            const t = (step - 10) / 15;
            testPointer.notify_absolute_motion(now(), $2 + ($4 - $2) * Math.min(t, 1), $3 + ($5 - $3) * Math.min(t, 1));
            if (step < 32)
                return GLib.SOURCE_CONTINUE;
            testPointer.notify_button(now(), 1, Clutter.ButtonState.RELEASED);
            return GLib.SOURCE_REMOVE;
        });
        'ok'" > /dev/null
    ;;
run)
    environment
    shift
    WAYLAND_DISPLAY=$display XDG_RUNTIME_DIR=$base/run nohup "$@" > "$base/app.log" 2>&1 &
    echo $! >> "$base/apps.pid"
    ;;
close-apps)
    # Only the apps "run" started (never by name: that could hit the same
    # app on the real desktop)
    if [[ -f $base/apps.pid ]]; then
        while read -r pid; do
            kill "$pid" 2> /dev/null || true
        done < "$base/apps.pid"
        rm -f "$base/apps.pid"
    fi
    ;;
log)
    cat "$base/shell.log"
    ;;
stop)
    # Everything started on the test's D-Bus session: the shell, the
    # services it started (portals, dconf), apps
    if [[ -f $base/bus ]]; then
        bus=$(cat "$base/bus")
        for environ in /proc/[0-9]*/environ; do
            if [[ -O $environ ]] && grep -qzx "DBUS_SESSION_BUS_ADDRESS=$bus" "$environ" 2> /dev/null; then
                pid=${environ#/proc/}
                kill "${pid%/environ}" 2> /dev/null || true
            fi
        done
    fi
    for pid in "$base/shell.pid" "$base/bus.pid"; do
        if [[ -f $pid ]]; then
            kill "$(cat "$pid")" 2> /dev/null || true
        fi
    done
    # The document portal mounts a folder in the runtime folder
    if mountpoint -q "$base/run/doc" 2> /dev/null; then
        fusermount3 -u "$base/run/doc" 2> /dev/null || true
    fi
    rm -f "$base/shell.pid" "$base/bus.pid" "$base/bus"
    ;;
*)
    sed -n '2,/^set -euo/p' "$0" | sed '$d'
    exit 1
    ;;
esac
