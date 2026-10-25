# Patrone branding

## Logo concept 01

![Logo concept 01](concepts/logo-concept-01.webp)

The concept has two forms.

**Primary logo (top).** A slanted geometric "P" with a blue gradient, the wide
"PATRONE" wordmark in white (the A has no crossbar), and "LINUX" in thin,
widely spaced blue capitals between two rules. Intended for:

- the login screen and lock screen
- the boot splash (Plymouth)
- the installer and its slideshow
- the About dialog, the website and the ISO's volume artwork

**Terminal logo (bottom).** The same mark drawn as dashed ASCII-style lines.
Intended for:

- the logo shown by system-info tools such as `fastfetch` (see below)
- the text-console login banner (`/etc/issue`)
- text-mode boot and recovery screens

The image here is a picture of ASCII art. The real terminal logo has to be
written as actual text characters so terminals can print it.

## Wallpaper 01

![Wallpaper 01](wallpapers/patrone-back-01-8k.png)

The default desktop and lock-screen wallpaper: the primary logo centered on a
near-black field with blue light streaks and faint terminal details in the
corners. It uses the same palette as the logo.

| File | Size | Use |
| --- | --- | --- |
| `wallpapers/patrone-back-01.png` | 1672×941 | Original artwork, kept as the source |
| `wallpapers/patrone-back-01-8k.png` | 7680×4320 | The 8K wallpaper the OS ships |

The 8K version is a lossless PNG on purpose. JPEG, even at quality 98, leaves
faint blotches in the dark blue gradients.

It was made from the original with `tools/upscale_wallpaper.py`.
It enlarges the glows and gradients smoothly, rebuilds the logo, text and
thin lines with sharp edges at full resolution, and adds fine grain so the
dark gradients don't band. Re-run it if the original changes:

```
python tools/upscale_wallpaper.py wallpapers/patrone-back-01.png wallpapers/patrone-back-01-8k
```

An upscale can sharpen edges but can't invent detail the original never had.
If the original design file, or a higher-resolution render of it, exists, a
native 8K export from that would be better still.

## fastfetch

`fastfetch/config.jsonc` is the system-wide fastfetch setup (installed as
`/etc/xdg/fastfetch/config.jsonc`, a path fastfetch 2.57 searches). It shows
`fastfetch/logo.txt` in Patrone Blue and Highlight, with the keys in
Highlight. To preview it from the repo:

```
fastfetch -c branding/fastfetch/config.jsonc --logo branding/fastfetch/logo.txt
```

**`logo.txt` is a placeholder** (a slanted block "P" over the wordmark). It
gets replaced by the terminal logo from the concept, redrawn as text. `$1`,
`$2` and `$3` in the file switch between Patrone Blue, Highlight and white.

## Colors measured from the concept

| Role | Hex | Where it appears |
| --- | --- | --- |
| Patrone Blue (primary) | `#288BFA` | Main body of the P |
| Highlight | `#4DA0FC` | Top-right of the P's bowl |
| Mid | `#2372DD` | Transition across the P |
| Deep | `#1851A8` | Lower stem of the P |
| Shadow | `#134394` | Darkest end of the gradient |
| Rule line / "LINUX" | `#3E91DE` | Thin rules and subtitle |
| Background | `#030303` | Near-black backdrop |
| Wordmark | `#FFFFFF` | "PATRONE" |

These come from a raster image, so treat them as a starting point. The vector
master becomes the source of truth once it exists.

## Still needed before release

- Vector (SVG) masters of the mark and the full logo.
- A mark-only version that stays readable at 16–32 px, for the app launcher
  button, panel and favicon.
- Single-color versions: white on dark, dark on light, and a GNOME-style
  symbolic icon.
- The wordmark typeface: identify a font whose license allows redistribution,
  or draw the letters as outlines.
- Name check: the word "Linux" in a product name is a trademark of Linus
  Torvalds, administered by the Linux Foundation. Check whether a public release
  needs their sublicense.
