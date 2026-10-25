# Patrone compositor

The Wayland compositor of Patrone Desktop, written in Rust with Smithay. For
now it runs as a window inside another desktop (GNOME, in the dev VM), so it
can be rebuilt and tried in seconds.

- **Wallpaper:** the Patrone wallpaper, scaled to fill the window, on
  Patrone's near-black `#030303`.
- **A terminal on start,** inside the compositor: `patrone-terminal
  --standalone`, else `ptyxis --standalone`, else `foot`. A terminal that
  exits with an error within two seconds is skipped for the next one.
- **Windows** can be moved, resized and closed. GTK apps draw their own title
  bars, and dragging those and the window borders works as usual.
- **Maximize and fullscreen,** from the window's own buttons, a double-click
  on its title bar, an app's F11, or the shortcuts below. Restoring puts the
  window back where it was, at its old size. Leaving fullscreen goes back to
  maximized if it was. Dragging a maximized window's title bar restores it
  under the pointer. Minimizing isn't offered, so GTK greys out that button.
- Click to focus. New windows open in the middle, focused, each a little
  below and to the right of the last. Menus and other popups are kept on
  screen.
- Apps draw their own title bars. Apps that ask through `xdg-decoration` are
  told to do so.

| Keys | Action |
| --- | --- |
| Super+left-drag | Move a window |
| Super+right-drag | Resize a window from the nearest corner |
| Super+Up | Maximize or restore the focused window |
| Super+F11 | Fullscreen or restore the focused window |
| Super+Q | Close the focused window |
| Super+Enter | Open another terminal |

Inside GNOME, run with `--modifier alt` to use Alt instead of Super: GNOME
keeps Super+drag for itself and moves the whole compositor window.

## Build and try it

On the dev VM, from a terminal in the GNOME session:

```
cargo run --manifest-path compositor/Cargo.toml -- --modifier alt
cargo test --manifest-path compositor/Cargo.toml
```

```
  --terminal COMMAND        Start COMMAND (run by sh) instead of the default terminal
  --wallpaper PATH          Draw PATH as the background (PNG)
  --modifier super|alt      Key held for window shortcuts (default: super)
  --screenshot PATH         Save a rendered frame to PATH (PNG), for debugging
  --screenshot-after SECS   When to save it (default: 5)
```

The default wallpaper is `branding/wallpapers/patrone-back-01.png` in this
repository. `RUST_LOG` changes the logging, for example `RUST_LOG=debug`.

From SSH, point it at the desktop session first:

```
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus WAYLAND_DISPLAY=wayland-0
```

`--screenshot` saves what the compositor drew, which works even when you
can't see its window: over SSH, or while the screen is locked.

`tools/window-states.py` is a GTK test window that asks to be maximized,
fullscreened and restored on a timer, and prints the state and size it gets
back, so window management can be checked without any input:

```
cargo run -- --terminal "python3 tools/window-states.py"
```

## How it draws

Smithay's winit backend opens the window, and frames are drawn with OpenGL ES.
The dev VM has no GPU, so that's Mesa's llvmpipe on the CPU. It's fast enough,
and costs almost nothing while nothing changes. While Mesa looks for a GPU it
prints `libEGL warning: failed to get driver name for fd -1` and a ZINK error;
they're harmless.

The compositor times its own frames, at most 60 a second and only when
something changed, with vsync off. GNOME stops sending frame callbacks while
its screen is locked, and waiting for them froze the compositor.

## Known limits

- It only runs nested. Running as a real session needs a DRM/KMS and libinput
  backend (milestone 3).
- One output. No layer-shell (panels), no Xwayland, and no server-side title
  bars, so apps that don't draw their own get none.
- No minimizing: there's no panel to bring a window back from yet.
- The keyboard layout is xkb's default (US) unless `XKB_DEFAULT_LAYOUT` and
  friends are set.
- The clipboard isn't shared with the desktop around the compositor. There's
  no primary selection yet, so middle-click paste only works within one app.

## Code

| File | What it does |
| --- | --- |
| `src/main.rs` | Startup and the event loop |
| `src/cli.rs` | Command-line options |
| `src/state.rs` | Wayland globals, windows, focus, where new windows go |
| `src/handlers.rs` | The Wayland protocols: surfaces, xdg-shell, xdg-decoration, seat, clipboard, cursor shapes |
| `src/input.rs` | Keyboard and pointer input, shortcuts |
| `src/grabs.rs` | Moving and resizing windows with the pointer |
| `src/maximize.rs` | Maximized and fullscreen windows |
| `src/winit.rs` | The nested window, drawing frames, screenshots |
| `src/wallpaper.rs` | The background |
| `src/launch.rs` | Starting terminals |

The structure follows Smithay's `smallvil` example, which is MIT-licensed
like Smithay itself; its notice is in `NOTICE`.
