"""Kerning: classes of letters by the shape of their sides (accented
letters join their letter's class), and the pairs between them that
interface text needs most."""

import unicodedata

import glyph as G

# a class for a letter's right side (the first of a pair), and for its left
# side (the second): letters whose sides are alike, by their base letter
RIGHT = {
    "T": "T", "V": "V", "W": "W", "Y": "Y", "A": "A", "L": "L", "F": "F", "P": "P",
    "O": "O", "D": "O", "Q": "O", "K": "K", "X": "K",
    "r": "r", "v": "v", "w": "v", "y": "v", "f": "f", "k": "k", "x": "k",
    "o": "o", "e": "o", "b": "o", "p": "o", "c": "c",
    "quoteright": "quote", "quotedblright": "quote", "quotesingle": "quote", "quotedbl": "quote",
    "period": "period", "comma": "period", "ellipsis": "period",
    "hyphen": "hyphen", "endash": "hyphen", "emdash": "hyphen",
}
LEFT = {
    "T": "T", "V": "V", "W": "W", "Y": "Y", "A": "A", "J": "J",
    "O": "O", "C": "O", "G": "O", "Q": "O",
    "o": "o", "c": "o", "d": "o", "e": "o", "g": "o", "q": "o", "oe": "o", "eth": "o",
    "a": "a", "ae": "a", "s": "s",
    "n": "n", "m": "n", "p": "n", "r": "n", "u": "n", "dotlessi": "n", "eng": "n",
    "v": "v", "w": "v", "y": "v", "x": "x", "z": "z",
    "period": "period", "comma": "period", "ellipsis": "period",
    "quoteleft": "quote", "quotedblleft": "quote", "quotesingle": "quote", "quotedbl": "quote",
    "hyphen": "hyphen", "endash": "hyphen", "emdash": "hyphen",
}
# letters whose accent or mark changes a side too much to share a class
APART = {"Lcaron", "dcaron", "tcaron", "lcaron", "Ldot", "ldot", "AE", "Thorn"}

PAIRS = {
    ("T", "o"): -72, ("T", "a"): -66, ("T", "s"): -60, ("T", "n"): -50, ("T", "v"): -40,
    ("T", "x"): -40, ("T", "z"): -44, ("T", "period"): -80, ("T", "A"): -56, ("T", "hyphen"): -54,
    ("T", "O"): -12, ("T", "J"): -40,
    ("V", "o"): -42, ("V", "a"): -42, ("V", "s"): -36, ("V", "n"): -20, ("V", "A"): -50,
    ("V", "period"): -72, ("V", "O"): -12, ("V", "hyphen"): -24, ("V", "J"): -30,
    ("W", "o"): -26, ("W", "a"): -26, ("W", "A"): -32, ("W", "period"): -50, ("W", "hyphen"): -14,
    ("Y", "o"): -72, ("Y", "a"): -62, ("Y", "s"): -56, ("Y", "n"): -44, ("Y", "A"): -60,
    ("Y", "period"): -82, ("Y", "O"): -20, ("Y", "hyphen"): -46, ("Y", "v"): -24, ("Y", "J"): -44,
    ("A", "T"): -56, ("A", "V"): -50, ("A", "W"): -32, ("A", "Y"): -60, ("A", "v"): -26,
    ("A", "quote"): -64, ("A", "O"): -12,
    ("L", "T"): -72, ("L", "V"): -62, ("L", "W"): -42, ("L", "Y"): -78, ("L", "quote"): -96,
    ("L", "v"): -34, ("L", "O"): -18, ("L", "hyphen"): -40,
    ("F", "A"): -40, ("F", "period"): -72, ("F", "o"): -24, ("F", "a"): -24, ("F", "J"): -40,
    ("P", "A"): -48, ("P", "period"): -92, ("P", "o"): -18, ("P", "a"): -18, ("P", "J"): -46,
    ("O", "A"): -12, ("O", "V"): -12, ("O", "Y"): -20, ("O", "T"): -12, ("O", "period"): -26,
    ("K", "o"): -24, ("K", "O"): -18, ("K", "v"): -30, ("K", "hyphen"): -40,
    ("r", "period"): -56, ("r", "a"): -10, ("r", "o"): -8, ("r", "hyphen"): -20,
    ("v", "period"): -48, ("v", "o"): -10, ("v", "a"): -10,
    ("k", "o"): -14,
    ("o", "v"): -10, ("o", "period"): -10, ("o", "x"): -10,
    ("c", "o"): -4,
    ("f", "quote"): 24, ("f", "o"): -12, ("f", "period"): -34,
    ("quote", "A"): -64, ("quote", "o"): -40, ("quote", "a"): -32, ("quote", "J"): -60,
    ("period", "quote"): -64, ("period", "T"): -80, ("period", "V"): -72, ("period", "Y"): -82,
    ("period", "W"): -50, ("period", "v"): -42,
    ("hyphen", "T"): -54, ("hyphen", "V"): -24, ("hyphen", "Y"): -46, ("hyphen", "A"): -10,
}


def base_of(name, unicodes):
    """the letter a glyph's sides come from: its own name, or its base
    letter's for accented letters"""
    if name in APART:
        return None
    if not unicodes:
        return name
    ch = chr(unicodes[0])
    base = unicodedata.normalize("NFD", ch)[0]
    if base != ch:
        for n, (u, _, _) in G.GLYPHS.items():
            if ord(base) in u:
                return n
    return name


def apply(font, master):
    """kerning groups and pairs for one master (heavier weights kern a
    little more: their letters reach further)"""
    scale = master.t(0.94, 1.0, 1.12)
    left, right = {}, {}
    for name, (unicodes, _, _) in G.GLYPHS.items():
        base = base_of(name, unicodes)
        if base is None:
            continue
        if base in RIGHT:
            right.setdefault(RIGHT[base], []).append(name)
        if base in LEFT:
            left.setdefault(LEFT[base], []).append(name)
    for cls, names in right.items():
        font.groups[f"public.kern1.{cls}"] = names
    for cls, names in left.items():
        font.groups[f"public.kern2.{cls}"] = names
    for (a, b), value in PAIRS.items():
        if a in right and b in left:
            font.kerning[(f"public.kern1.{a}", f"public.kern2.{b}")] = round(value * scale)
