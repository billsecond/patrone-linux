# Patrone Terminal

The default terminal of Patrone Linux: GTK4 and VTE, written in Rust.

- **Copy on select:** selected text goes to the clipboard as soon as you stop
  selecting.
- **Paste on right-click.** Middle-click pastes the last selection, as usual
  on Linux.
- **Patrone Dark:** a near-black theme with the Patrone blues, in JetBrains
  Mono (Fira Code if JetBrains Mono is missing).
- Tabs, 100,000 lines of scrollback, and xterm-style `-e` and `-T` options so
  it can be the `x-terminal-emulator` alternative.

| Keys | Action |
| --- | --- |
| Ctrl+Shift+C / Ctrl+Shift+V | Copy / paste |
| Shift+Insert | Paste |
| Ctrl+Shift+T / Ctrl+Shift+N | New tab / new window, in the current directory |
| Ctrl+Shift+W | Close tab |
| Ctrl+Page Down / Ctrl+Page Up | Next / previous tab |
| Ctrl+Plus / Ctrl+Minus / Ctrl+0 | Bigger / smaller / normal text |

New tabs open in the current tab's directory when the shell reports it with
the OSC 7 escape sequence. Patrone's `zsh/zshrc` does.

## Build and try it

On the dev VM (the libraries come from `scripts/bootstrap-dev-vm.sh`):

```
cargo run --manifest-path terminal/Cargo.toml
cargo test --manifest-path terminal/Cargo.toml
```

To install it for your user, with a launcher entry in GNOME:

```
./terminal/install-dev.sh             # the app
./terminal/install-dev.sh --default   # and make it GNOME's default terminal
./terminal/install-dev.sh --zshrc     # and use Patrone's zshrc
```

## The shell

`zsh/zshrc` is Patrone's default zsh setup, shipped later in
`patrone-shell-config` as `/etc/skel/.zshrc`. It draws a two-line prompt,
blue for normal users and red for root:

```
┌──(user㉿host)-[~/src]
└─$
```

It also turns on completion menus, shared history, `zsh-autosuggestions` and
`zsh-syntax-highlighting`.
