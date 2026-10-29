# Patrone Sans

PatroneOS's interface typeface, a humanist sans of its own: open, clear
letters drawn for screens, in the family of styles Noto Sans and other
interface fonts belong to, with nothing taken from any of them. Every
letter is drawn by code in `source/`: a pen stroke along a skeleton,
outlined. `FONTLOG.txt` says how it was made and why it looks as it does.

![Patrone Sans 1 and 2 at 9, 10 and 11 pt](comparison.png)

| File | What |
| --- | --- |
| `ttf/PatroneSans[wght].ttf` | The font: a variable font with one axis, weight, 300 to 900, and six named weights: Light, Regular, Medium, SemiBold, Bold, Black |
| `OFL.txt` | The licence, SIL Open Font License 1.1 |
| `FONTLOG.txt` | How the font was made, what it covers, its changes |
| `source/` | The drawing: `engine.py` (strokes and their outlines), `glyph.py` (the weights' numbers, spacing), `parts.py` (arches, bowls, diagonals), `lower.py`, `upper.py`, `figures.py`, `punct.py`, `marks.py`, `composites.py` (accented letters), `kerning.py`, and `make_ufos.py`, which writes the UFO sources |
| `build.sh` | Builds `ttf/` from `source/` |
| `requirements.txt` | The build tools' versions |
| `tools/finish.py` | The last step: overlap flags, STAT, gasp, names checked |
| `tools/preview.py` | Draws the masters big, straight from the code, while drawing |
| `tools/comparison_sheet.py` | Draws `comparison.png` |
| `tools/proof.py` | Draws text big through Pango, to look at letters |
| `fontbakery-report.txt` | fontbakery's report on `ttf/` |

The package is `fonts-patrone-sans` (`packaging/fonts-patrone-sans`),
which installs the font in `/usr/share/fonts/truetype/patrone-sans/`.
Patrone Settings (Theme → Font) and GNOME Settings' Patrone page switch
the desktop between Patrone Sans and Noto Sans.

## The design

- **Humanist, for screens**: a large x-height (530 of 1000 units, capitals
  710, ascenders 760), open apertures in a, c, e and s, round dots, and a
  little contrast: the pen is an oval wider than it is tall, so upright
  strokes are a little heavier than level ones, and strokes thin where
  they leave a stem.
- **Letters that can't be mistaken**: l has a tail, I is a plain stroke
  shorter than l, and 1 has a flag and a foot, so I, l, 1 and | are four
  different shapes. The zero is plain, no slash: it's a narrow oval, O a
  wide round one, about half as wide again.
- **Six weights** in one variable font, Light (300) to Black (900). Heavy
  weights widen their letters and tighten their counters, and some letters
  (s, a, e, ß) change shape a little so their curves stay rounder than the
  pen.
- **Figures** are tabular, every one the same width, so clocks and columns
  of numbers don't shift.
- **The same line spacing as Patrone Sans 1** (ascender 1069, descender
  293, no line gap), so switching fonts moves nothing on screen.

## What it covers

404 glyphs: Basic Latin, Latin-1 and Latin Extended-A in full (Western,
Central and Eastern European languages: Polish, Czech, Slovak, Hungarian,
Romanian, Croatian, Slovenian, the Baltic languages, Turkish and others),
Romanian's Ș ș Ț ț with a comma below, and the punctuation and signs
the desktop uses: dashes, quotes, ellipsis, bullets, ●, ∶ (the clock's
ratio), arrows, math signs, currency (€ £ ¥ ¢ ¤ ƒ), © ® ™, superscripts and
fractions. Accents also combine with any letter (mark positioning), and
capitals take flatter accents.

Not covered, so they come from Noto Sans, which fontconfig puts right after
Patrone Sans (`branding/system/fontconfig/05-patrone-fonts.conf`): Greek,
Cyrillic, Vietnamese's extra letters and everything else. No italic either:
apps that ask for one get a slanted Patrone Sans from fontconfig.

## Building

On the dev VM (or any Linux with python3 and curl):

```
fonts/build.sh
```

It makes a venv in `~/.cache/patrone-sans` (the first run needs the
network; no sudo), then:

1. `source/make_ufos.py` draws the three masters (Light, Regular, Black)
   and writes them as UFO sources with a designspace;
2. `fontmake` compiles the variable font, the six weights as named
   instances;
3. `tools/finish.py` finishes it;

and fontbakery's universal checks run (`--no-check` skips them). It takes
about a minute. Dates in the font come from the last commit to `source/`
(`SOURCE_DATE_EPOCH`), so a rebuild gives the same bytes.

`ttf/` is kept in the repository, since a package build can't fetch the
tools.

While drawing, `tools/preview.py` compiles the masters straight from the
code and draws them with FreeType (it needs freetype-py and Pillow too):

```
python3 fonts/tools/preview.py proof.png --big "aegs Rk" --text "minimum handgloves"
```

The comparison sheet needs the system's python3 (PyGObject and
fontTools), with Patrone Sans 1 installed:

```
python3 fonts/tools/comparison_sheet.py comparison.png fonts/ttf/PatroneSans[wght].ttf
```

## What was checked

- **fontbakery 1.1.0** (`check-universal`): 104 checks passed, no errors,
  fails or warnings (`fontbakery-report.txt`).
- The engine measures how far each outline strays from its stroke and
  splits the curves that stray more than 1.5 units, in every weight alike;
  a handful stay within 3 units, in tight turns.
- `comparison.png`: Patrone Sans 1 and 2 at 9, 10 and 11 pt, the six
  weights and tabular figures, light and dark, through Pango and cairo at
  96 dpi with GNOME's defaults, on the dev VM. Lines of text come out
  about 3% shorter than in Patrone Sans 1.
- Through Pango (`pango-view`): kerning, accents combined with letters
  (é, Á with the capital accent, ï and í on a dotless i, ș, ą, Ě), and
  every sign above.
- PatroneOS's own apps in a throwaway GNOME Shell on the dev VM, with only
  the new font as Patrone Sans: the taskbar, Start menu, Patrone
  Settings, GNOME Settings' Patrone page, File Manager and Task Manager.
  Every character the apps' sources use is in the font, apart from box
  drawing, ✗, ⚙ and ㉿ in the shell prompts and READMEs (the terminal's
  monospace font draws those) and two blocks in a test of Unpack's.

Not checked: Windows and macOS rendering, every language Latin Extended-A
serves, and Chromium and other apps that read fonts themselves.
