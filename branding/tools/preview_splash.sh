#!/bin/sh
# Shows the boot splash in every mode without rebooting, and saves a screen
# dump of each: boot, a message (as a file-system check sends), the
# disk-unlock password and a question (with typing), shutdown and restart
# with a message, and the four update modes at 31%.
#
#   sudo branding/tools/preview_splash.sh THEME OUT SIZE...
#   sudo branding/tools/preview_splash.sh out/patrone-branding_0.1.17_all.deb /tmp/splash 1920x1080 1024x768
#
# THEME is a theme folder or a patrone-branding .deb. Run it from a
# terminal (Plymouth needs one; it gets this one, not a VT). Each SIZE is an
# invisible X screen (Xvfb) for Plymouth's X11 renderer, the only renderer
# it can see: the theme and renderers are bind-mounted in a private mount
# namespace, so the real screen and the installed theme are untouched.
# Xvfb, the X11 renderer and xdotool are downloaded with apt-get download
# into /tmp/preview-splash-tools on first use, not installed. Then make
# contact sheets with preview_splash_sheets.py (Pillow and NumPy).
set -eu
if [ -z "${PREVIEW_SPLASH_NAMESPACE:-}" ]; then
    PREVIEW_SPLASH_NAMESPACE=1 exec unshare --mount --propagation private sh "$0" "$@"
fi
theme=$1 out=$2
shift 2
tty=$(tty)
tools=/tmp/preview-splash-tools
renderers=/usr/lib/x86_64-linux-gnu/plymouth/renderers
export LD_LIBRARY_PATH=$tools/root/usr/lib/x86_64-linux-gnu

if [ ! -x "$tools/root/usr/bin/Xvfb" ]; then
    mkdir -p "$tools/debs"
    (cd "$tools/debs" && apt-get download xvfb plymouth-x11 xdotool libxdo3)
    for deb in "$tools"/debs/*.deb; do
        dpkg-deb -x "$deb" "$tools/root"
    done
fi

work=$(mktemp -d)
case $theme in
    *.deb)
        dpkg-deb -x "$theme" "$work/package"
        theme=$work/package/usr/share/plymouth/themes/patrone
        ;;
esac
mkdir "$work/renderers"
cp "$tools/root$renderers/x11.so" "$work/renderers/"
mount --bind "$theme" /usr/share/plymouth/themes/patrone
mount --bind "$work/renderers" "$renderers"
mkdir -p "$out"

start() {
    plymouthd --no-daemon --tty="$tty" --no-boot-log --ignore-serial-consoles \
        --pid-file="$work/pid" --kernel-command-line="quiet splash" --mode="$1" &
    daemon=$!
    sleep 0.7
    plymouth --show-splash
}
shot() {
    sleep 1.2
    cp "$screen/Xvfb_screen0" "$out/$size-$1.xwd"
}
stop() {
    plymouth quit
    wait "$daemon" || true
}

for size in "$@"; do
    screen=$work/screen-$size
    mkdir "$screen"
    "$tools/root/usr/bin/Xvfb" :99 -screen 0 "${size}x24" -fbdir "$screen" -nolisten tcp -ac 2> /dev/null &
    xvfb=$!
    sleep 1
    export DISPLAY=:99
    "$tools/root/usr/bin/xdotool" mousemove 10 10

    start boot
    shot boot
    plymouth display-message --text="Checking file system on /dev/sda2: 42% complete"
    shot boot-message
    stop

    start boot
    plymouth ask-for-password --prompt="Please unlock disk sda3_crypt" > /dev/null &
    ask=$!
    sleep 0.5
    "$tools/root/usr/bin/xdotool" type --delay 50 secret
    shot boot-password
    kill "$ask" || true
    stop

    start boot
    plymouth ask-question --prompt="Type the recovery key" > /dev/null &
    ask=$!
    sleep 0.5
    "$tools/root/usr/bin/xdotool" type --delay 50 4821-77
    shot boot-question
    kill "$ask" || true
    stop

    for mode in shutdown reboot; do
        start "$mode"
        plymouth display-message --text="A stop job is running for Network Manager (3s / 1min 30s)"
        shot "$mode"
        stop
    done

    for mode in updates system-upgrade firmware-upgrade system-reset; do
        start "$mode"
        plymouth system-update --progress=31
        plymouth display-message --text="Installing firefox (3 of 12)"
        shot "$mode"
        stop
    done

    kill "$xvfb"
    wait "$xvfb" || true
done
rm -rf "$work"
gzip -f -1 "$out"/*.xwd
chmod -R a+rX "$out"
echo "$out: $(ls "$out" | wc -l) screen dumps"
