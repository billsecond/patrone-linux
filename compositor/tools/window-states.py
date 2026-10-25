#!/usr/bin/env python3
"""A GTK window that asks the compositor to maximize, fullscreen and restore
it on a timer, and prints the state and size it gets back. For checking the
compositor without sending it any input:

    patrone-compositor --terminal "python3 tools/window-states.py"
    patrone-compositor --terminal "python3 tools/window-states.py --start-maximized"
"""

import argparse
import time

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402

STEPS = [
    "maximize",
    "unmaximize",
    "fullscreen",
    "unfullscreen",
    "maximize",
    "fullscreen",
    "unfullscreen",  # back to maximized
    "unmaximize",
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--interval", type=float, default=2.0, help="seconds between steps")
    parser.add_argument("--start-maximized", action="store_true")
    args = parser.parse_args()

    start = time.monotonic()
    app = Gtk.Application(application_id="org.patrone.WindowStates")

    def report(window: Gtk.Window, what: str) -> None:
        print(
            f"{time.monotonic() - start:5.1f}s {what:<20} "
            f"maximized={window.is_maximized()!s:<5} fullscreen={window.is_fullscreen()!s:<5} "
            f"size={window.get_width()}x{window.get_height()}",
            flush=True,
        )

    def activate(app: Gtk.Application) -> None:
        window = Gtk.ApplicationWindow(application=app, title="Window states")
        window.set_default_size(640, 400)
        label = Gtk.Label(label="Window states test")
        window.set_child(label)
        if args.start_maximized:
            window.maximize()
        window.present()
        steps = iter(STEPS + ["quit"])
        previous = ["start"]

        def tick() -> bool:
            report(window, f"after {previous[0]}")
            step = next(steps)
            previous[0] = step
            label.set_label(step)
            if step == "quit":
                app.quit()
                return False
            getattr(window, step)()
            return True

        GLib.timeout_add(int(args.interval * 1000), tick)

    app.connect("activate", activate)
    app.run(None)


if __name__ == "__main__":
    main()
