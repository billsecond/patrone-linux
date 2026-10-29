# Patrone compositor

The Wayland compositor of Patrone Desktop, written in Rust with Smithay. It
runs either as a window inside another desktop (GNOME, in the dev VM), so it
can be rebuilt and tried in seconds, or on the screen itself, started from a
virtual terminal or the login screen.

- **Wallpaper:** the Patrone wallpaper, scaled to fill the screen, on
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
| Super+Shift+E | Quit the compositor |
| Ctrl+Alt+F1 to F12 | Switch virtual terminal (on the screen itself) |

Inside GNOME, run with `--modifier alt` to use Alt instead of Super: GNOME
keeps Super+drag for itself and moves the whole compositor window.

## Build and try it, nested

On the dev VM, from a terminal in the GNOME session:

```
cargo run --manifest-path compositor/Cargo.toml -- --modifier alt
cargo test --manifest-path compositor/Cargo.toml
```

```
  --backend auto|nested|drm Run as a window inside another desktop (nested) or
                            on the screen itself (drm). auto: nested when
                            WAYLAND_DISPLAY or DISPLAY is set
  --mode WIDTHxHEIGHT       Screen resolution for the drm backend
  --terminal COMMAND        Start COMMAND (run by sh) instead of the default terminal
  --wallpaper PATH          Draw PATH as the background (PNG)
  --modifier super|alt      Key held for window shortcuts (default: super)
  --screenshot PATH         Save a rendered frame to PATH (PNG), for debugging
  --screenshot-after SECS   When to save it (default: 5)
```

The default wallpaper is `branding/wallpapers/patrone-back-02.png` in this
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

## On the screen itself

Without `WAYLAND_DISPLAY` or `DISPLAY` the compositor takes over the screen
(the drm backend). It needs the session that's on the screen, so start it
from a virtual terminal: on the VM's console, press Ctrl+Alt+F3, log in, and
run

```
~/src/patrone-linux/compositor/target/release/patrone-compositor 2> ~/patrone.log
```

Super+Shift+E quits back to the text console, and Ctrl+Alt+F1 returns to the
login screen. Over SSH or inside GNOME it can't get the screen and says so.

To choose it at the login screen instead, run `./compositor/install-dev-session.sh`
(it uses sudo) and pick "Patrone" with the gear button. Logging in there as
a user who is already logged in elsewhere (for example through Remote Login)
may end that other session. The session's log is
`~/.local/state/patrone-session.log`.

On the screen, the compositor draws the cursor itself, from the cursor theme
(`XCURSOR_THEME` and `XCURSOR_SIZE`, else the system default). It uses the
screen's preferred mode, except on Hyper-V, whose "preferred" mode is a
1024×768 boot mode, so there the largest; `--mode` overrides both.

## How it draws

Both backends draw with OpenGL ES through Smithay's GLES renderer.

- **Nested:** Smithay's winit backend opens the window. The compositor times
  its own frames, at most 60 a second and only when something changed, with
  vsync off. GNOME stops sending frame callbacks while its screen is locked,
  and waiting for them froze the compositor.
- **On the screen:** DRM/KMS shows the frames, one per vblank, with buffers
  from GBM, and libinput delivers input. logind, through libseat, gives the
  compositor the screen and input devices and takes them away while another
  virtual terminal is shown.

The dev VM has no GPU, so Mesa draws on the CPU: llvmpipe nested, and its
kms_swrast driver on the screen, into linear buffers the display can show.
Smithay's anvil example refuses software rendering on the screen; Patrone
accepts it. While Mesa looks for a GPU it prints
`libEGL warning: failed to get driver name for fd -1`, a ZINK error and an
`eglInitialize` BAD_ALLOC; they're harmless.

## Known limits

- One GPU and one screen, chosen at start: no monitors plugged in later, no
  multi-monitor, scale 1.
- No layer-shell (panels), no Xwayland, and no server-side title bars, so
  apps that don't draw their own get none.
- No minimizing: there's no panel to bring a window back from yet.
- The keyboard layout is xkb's default (US) unless `XKB_DEFAULT_LAYOUT` and
  friends are set.
- The clipboard isn't shared with the desktop around a nested compositor.
  There's no primary selection yet, so middle-click paste only works within
  one app.

## Code

| File | What it does |
| --- | --- |
| `src/main.rs` | Startup, choosing a backend, the event loop |
| `src/cli.rs` | Command-line options |
| `src/state.rs` | Wayland globals, windows, focus, where new windows go |
| `src/handlers.rs` | The Wayland protocols: surfaces, xdg-shell, xdg-decoration, seat, clipboard, cursor shapes |
| `src/input.rs` | Keyboard and pointer input, shortcuts |
| `src/grabs.rs` | Moving and resizing windows with the pointer |
| `src/maximize.rs` | Maximized and fullscreen windows |
| `src/render.rs` | What's drawn on the output, and screenshots |
| `src/winit.rs` | The nested backend |
| `src/udev.rs` | The on-screen backend: DRM/KMS, GBM, libinput, the session |
| `src/cursor.rs` | Cursor images from the cursor theme |
| `src/wallpaper.rs` | The background |
| `src/launch.rs` | Starting terminals |
| `data/` | The login screen's session entry and its start script |

The structure follows Smithay's `smallvil` example, and the on-screen backend
Smithay's `anvil`; both are MIT-licensed like Smithay itself, and the notice
is in `NOTICE`.
