# Patrone Terminal

The default terminal of Patrone Linux: GTK4 and VTE, written in Rust.

- **Copy on select:** selected text goes to the clipboard as soon as you stop
  selecting.
- **Paste on right-click.** Middle-click pastes the last selection, as usual
  on Linux.
- **Links:** Ctrl+click opens web addresses, and links that programs mark with
  the OSC 8 escape sequence. They underline when the pointer is over them.
- **Asks before closing** a tab or window where a program other than the
  shell is still running, and names the program.
- **Patrone Dark:** a near-black theme with the Patrone blues, in JetBrains
  Mono.
- **Preferences** (menu, or Ctrl+Comma): font, lines of scrollback, and
  switches for copy on select and paste on right-click. They're saved in
  `~/.config/patrone-terminal/settings.ini` and apply to open terminals at
  once.
- Tabs, 100,000 lines of scrollback, and xterm-style `-e` and `-T` options so
  it can be the `x-terminal-emulator` alternative.

| Keys | Action |
| --- | --- |
| Ctrl+Shift+C / Ctrl+Shift+V | Copy / paste |
| Shift+Insert | Paste |
| Ctrl+click | Open a link |
| Ctrl+Shift+T / Ctrl+Shift+N | New tab / new window, in the current directory |
| Ctrl+Shift+W | Close tab |
| Ctrl+Page Down / Ctrl+Page Up | Next / previous tab |
| Ctrl+Plus / Ctrl+Minus / Ctrl+0 | Bigger / smaller / normal text |
| Ctrl+Comma | Preferences |

New tabs open in the current tab's directory when the shell reports it with
the OSC 7 escape sequence. Patrone's `zsh/zshrc` does.

All windows normally share one process: running `patrone-terminal` again
opens a window in the one already running. `--standalone` (`-s`) starts a
separate process, for example inside a nested compositor.

## Build and try it

On the dev VM (the libraries come from `scripts/bootstrap-dev-vm.sh`):

```
cargo run --manifest-path terminal/Cargo.toml
cargo test --manifest-path terminal/Cargo.toml
```

To install it for your user, with a launcher entry and icon in GNOME:

```
./terminal/install-dev.sh             # the app
./terminal/install-dev.sh --default   # and use it instead of Ubuntu's terminal
./terminal/install-dev.sh --zshrc     # and use Patrone's zshrc and zprofile
```

`--default` points Ctrl+Alt+T and "Open in Terminal" at Patrone Terminal
(through `~/.config/xdg-terminals.list`), swaps it into the dock, and hides
Ubuntu's terminal (Ptyxis) from the app grid. Deleting
`~/.local/share/applications/org.gnome.Ptyxis.desktop` brings that back.

The package is built with `packaging/build.sh patrone-terminal` (see
`packaging/README.md`). The crate keeps `rust-version` at Ubuntu's rustc
(1.93), which builds the package.

To try it without touching a desktop session, GTK's Broadway backend draws it
in a web page: run `gtk4-broadwayd --port 8087 :7`, start the terminal with
`GDK_BACKEND=broadway BROADWAY_DISPLAY=:7`, and open `http://localhost:8087`
(tunnel the port over SSH from Windows). Mouse input works fully; letter keys
typed by browser automation don't arrive.

## The shell

`zsh/zshrc` is Patrone's default zsh setup, shipped in `patrone-shell-config`
as `/etc/skel/.zshrc`. It draws a two-line prompt, blue for normal users and
red for root:

```
┌──(user㉿host)-[~/src]
└─$
```

It also turns on completion menus, shared history, `zsh-autosuggestions` and
`zsh-syntax-highlighting`, shows a failed command's exit status on the right,
and reports the directory to the terminal. `zsh/zprofile` makes login shells
read `~/.profile`, where Ubuntu keeps PATH additions such as `~/.local/bin`.

The terminal runs your login shell. To switch yours to zsh:
`chsh -s /usr/bin/zsh`, then log out and back in.
