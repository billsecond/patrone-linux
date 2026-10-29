#!/usr/bin/env python3
# Makes Ubuntu's first-login setup (gnome-initial-setup) Patrone's, in its
# unpacked source, for gnome-initial-setup/build.sh (README.md there):
#
#   patrone.py SOURCE IMAGES PICTURE
#
# IMAGES has Patrone's pictures (branding/tools/make_installer_images.sh);
# PICTURE is patrone-software.svg (make_software_picture.py).
#
# - Patrone Linux where Ubuntu's pages say Ubuntu.
# - The "Get started with more applications" page offers Patrone Software,
#   with the Craft apps, in place of Ubuntu's App Center.
# - Patrone's P in place of Ubuntu's logo and mascot, and every other picture
#   in Patrone's blue where it's in Ubuntu's orange (the same rule as the
#   installer's, installer/patch_source.py).
# - No calls to the crash reporter's settings, which asked for a password.
# - No "View release notes" link, which opened Ubuntu's.
import colorsys
import re
import sys
from pathlib import Path

source, images, picture = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
pages = source / 'gnome-initial-setup/pages'


def edit(path, old, new):
    text = path.read_text(encoding='utf-8')
    assert old in text, f'{path}: not found: {old!r}'
    path.write_text(text.replace(old, new), encoding='utf-8')


# The words
edit(pages / 'appearance/gis-appearance-page.blp',
     '_("Choose how Ubuntu looks")', '_("Choose how Patrone Linux looks")')
edit(pages / 'welcome/gis-welcome-page.c', 'g_strdup ("Ubuntu")', 'g_strdup ("Patrone Linux")')
edit(pages / 'welcome/gis-welcome-page.ui', 'Welcome to Ubuntu 24.04 LTS', 'Welcome to Patrone Linux')
# "View release notes" opened Ubuntu's release notes for the system's
# version (Ubuntu's notes for "0.1"): hidden until Patrone has its own
# (docs/COMPLIANCE.md, item 18)
edit(pages / 'welcome/gis-welcome-page.c',
     'gtk_widget_set_visible (priv->release_notes, !!version);',
     'gtk_widget_set_visible (priv->release_notes, FALSE);')
edit(pages / 'apps/gis-apps-page.blp',
     '''_("Ubuntu's App Center has a range of apps you can get started with.")''',
     '''_("Patrone Software has the Craft apps, and thousands more from Flathub.")''')
edit(pages / 'apps/gis-apps-page.blp', '_("Open App Center")', '_("Open Patrone Software")')
# Patrone Software (GNOME Software) first: App Center isn't on Patrone
edit(pages / 'apps/gis-apps-page.c',
     'storecmd = g_find_program_in_path ("snap-store");',
     'storecmd = g_find_program_in_path ("gnome-software");')

# Ubuntu's page about sending reports (ubuntu_insights), which Patrone skips
# (patrone-desktop's vendor.conf), is still made, out of sight, and kept
# calling the crash reporter's settings service (whoopsie-preferences).
# Without the crash reporter, which patrone-desktop removes, that asked for
# an administrator's password: five times in one first-login setup
# (2026-10-27). With no crash reporter there's nothing to set.
edit(pages / 'ubuntu-insights/gis-ubuntu-insights-page.c',
     '  g_dbus_proxy_new_for_bus (G_BUS_TYPE_SYSTEM, G_DBUS_PROXY_FLAGS_NONE, NULL,\n'
     '                            "com.ubuntu.WhoopsiePreferences",',
     '  /* Patrone: no crash reporter (whoopsie), so nothing to set */\n'
     '  return;\n'
     '\n'
     '  g_dbus_proxy_new_for_bus (G_BUS_TYPE_SYSTEM, G_DBUS_PROXY_FLAGS_NONE, NULL,\n'
     '                            "com.ubuntu.WhoopsiePreferences",')


def sized_like(svg, original):
    """svg with original's width and height on its <svg>, so it fills the
    same space."""
    head = re.search(r'<svg\b[^>]*>', original.read_text(encoding='utf-8')).group(0)
    size = ''.join(f' {m.group(0)}' for m in re.finditer(r'\b(width|height)="[^"]*"', head))
    text = svg.read_text(encoding='utf-8')
    root = re.search(r'<svg\b[^>]*>', text)
    bare = re.sub(r'\s(width|height)="[^"]*"', '', root.group(0))
    return text[:root.start()] + bare.replace('<svg', f'<svg{size}', 1) + text[root.end():]


# The pictures that are Ubuntu's own
for target, new in {
    pages / 'apps/app-center.svg': picture,
    pages / 'language/distributor-logo.svg': images / 'patrone-mark.svg',
    pages / 'welcome/ubuntu-mascot-light.svg': images / 'patrone-mark.svg',
    pages / 'welcome/ubuntu-mascot-dark.svg': images / 'patrone-mark.svg',
}.items():
    text = sized_like(new, target)
    target.write_text(text, encoding='utf-8')
print('Patrone Software, the P and the words in place of Ubuntu\'s')

PATRONE_BLUE = '#288bfa'
BLUE_HUE = 212 / 360


def recolor(match):
    text = match.group(0)
    if text.lower() == '#e95420':  # Ubuntu Orange itself
        return PATRONE_BLUE
    r, g, b = (int(text[i:i + 2], 16) / 255 for i in (1, 3, 5))
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    if not ((h < 45 / 360 or h > 345 / 360) and s > 0.35 and 0.08 < l < 0.95):
        return text
    blue = colorsys.hls_to_rgb(BLUE_HUE, l, min(1.0, s * 1.05))
    return '#' + ''.join(f'{round(c * 255):02x}' for c in blue)


recolored = []
for svg in sorted(pages.rglob('*.svg')):
    if svg.parent.name in ('ubuntu-pro', 'ubuntu-insights'):
        continue  # pages Patrone leaves out (patrone-desktop's vendor.conf)
    text = svg.read_text(encoding='utf-8')
    new = re.sub(r'#[0-9A-Fa-f]{6}\b', recolor, text)
    if new != text:
        svg.write_text(new, encoding='utf-8')
        recolored.append(svg.relative_to(pages).as_posix())
print(f'{len(recolored)} pictures in Patrone blue: {", ".join(recolored)}')
