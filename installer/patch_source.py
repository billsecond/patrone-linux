#!/usr/bin/env python3
# Makes Ubuntu's installer front end Patrone's, in its source, for
# installer/build.sh (installer/README.md says why each):
#
#   patch_source.py SOURCE IMAGES
#
# SOURCE is ubuntu-desktop-provision's checkout; IMAGES has Patrone's
# pictures (branding/tools/make_installer_images.sh): patrone-logo.svg,
# patrone-logo-dark.svg and patrone-mark.svg.
#
# - Ubuntu's logo, mascot and disk icon become Patrone's wordmark and P.
# - Every other picture's Ubuntu orange becomes Patrone's blue, lighter and
#   darker shades alike, so the drawings keep their shading.
# - Ubuntu's spinner (Yaru's, on the Next button among others) becomes
#   Flutter's plain one: a ring that turns, in Patrone's blue.
# - The username it suggests is the first initial and last name
#   ("William Daugherty": "wdaugherty"), not the whole name run together.
# - Patrone's licence agreement gets a page of its own after "Try or
#   install" (patrone_licence_page.dart).
import colorsys
import re
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image

source, images = Path(sys.argv[1]), Path(sys.argv[2])
provision = source / 'packages/ubuntu_provision'
bootstrap = source / 'apps/ubuntu_bootstrap'

# Ubuntu's own pictures, and Patrone's in their place
for target, picture in {
    provision / 'assets/images/logo-light.svg': 'patrone-logo.svg',
    provision / 'assets/images/logo-dark.svg': 'patrone-logo-dark.svg',
    provision / 'assets/images/mascot-light.svg': 'patrone-mark.svg',
    provision / 'assets/images/mascot-dark.svg': 'patrone-mark.svg',
    provision / 'assets/images/ubuntu_storage_icon.svg': 'patrone-mark.svg',
    bootstrap / 'assets/storage/ubuntu.svg': 'patrone-mark.svg',
    bootstrap / 'assets/welcome/logo-light.svg': 'patrone-logo.svg',
    bootstrap / 'assets/welcome/logo-dark.svg': 'patrone-logo-dark.svg',
}.items():
    assert target.exists(), target
    shutil.copyfile(images / picture, target)

PATRONE_BLUE = '#288bfa'
BLUE_HUE = 212 / 360  # Patrone Blue's


def orange(r, g, b):
    """Ubuntu's oranges (and its red-orange darks): a warm, saturated hue."""
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    return (h < 45 / 360 or h > 345 / 360) and s > 0.35 and 0.08 < l < 0.95


def to_blue(r, g, b):
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    return colorsys.hls_to_rgb(BLUE_HUE, l, min(1.0, s * 1.05))


def recolor_hex(match):
    text = match.group(0)
    if text.lower() == '#e95420':  # Ubuntu Orange itself
        return PATRONE_BLUE
    r, g, b = (int(text[i:i + 2], 16) / 255 for i in (1, 3, 5))
    if not orange(r, g, b):
        return text
    return '#' + ''.join(f'{round(c * 255):02x}' for c in to_blue(r, g, b))


svgs = recolored = 0
for svg in sorted(list((provision / 'assets').rglob('*.svg')) + list((bootstrap / 'assets').rglob('*.svg'))):
    if '/slides/' in svg.as_posix():
        continue  # patrone-branding's slides take their place
    text = svg.read_text(encoding='utf-8')
    new = re.sub(r'#[0-9A-Fa-f]{6}\b', recolor_hex, text)
    svgs += 1
    if new != text:
        svg.write_text(new, encoding='utf-8')
        recolored += 1
print(f'{recolored} of {svgs} pictures recolored')

# The theme previews (light-theme.png, dark-theme.png), pixel by pixel
for png in sorted((provision / 'assets/theme').glob('*.png')):
    image = Image.open(png).convert('RGBA')
    pixels = np.asarray(image, dtype=float) / 255
    rgb = pixels[..., :3]
    hls = np.vectorize(colorsys.rgb_to_hls, otypes=[float, float, float])(*rgb.transpose(2, 0, 1))
    h, l, s = (np.asarray(c) for c in hls)
    warm = ((h < 45 / 360) | (h > 345 / 360)) & (s > 0.35) & (l > 0.08) & (l < 0.95)
    blue = np.stack(np.vectorize(colorsys.hls_to_rgb, otypes=[float, float, float])(
        np.full_like(h, BLUE_HUE), l, np.minimum(1.0, s * 1.05)), axis=-1)
    rgb[warm] = blue[warm]
    Image.fromarray((pixels * 255).round().astype(np.uint8), 'RGBA').save(png)
    print(f'{png.name}: {int(warm.sum())} pixels recolored')


def edit(path, old, new, count=1):
    text = path.read_text(encoding='utf-8')
    assert text.count(old) >= count, f'{path}: not found: {old!r}'
    path.write_text(text.replace(old, new), encoding='utf-8')


# Flutter's spinner in place of Yaru's
spinners = 0
for dart in sorted(list(source.glob('packages/*/lib/**/*.dart')) + list((bootstrap / 'lib').rglob('*.dart'))):
    text = dart.read_text(encoding='utf-8')
    if 'YaruCircularProgressIndicator(' in text:
        assert 'package:flutter/material.dart' in text, dart
        spinners += text.count('YaruCircularProgressIndicator(')
        dart.write_text(text.replace('YaruCircularProgressIndicator(', 'CircularProgressIndicator('), encoding='utf-8')
print(f'{spinners} spinners')

# The username it suggests
edit(provision / 'lib/src/identity/identity_model.dart',
     '''  /// The current username or a sanitized real name if not set.
  String get username =>
      _username.value ??
      realName.sanitize().toLowerCase().truncate(kMaxUsernameLength);''',
     '''  /// The current username or, if not set, the real name's first initial
  /// and last name ("William Daugherty": "wdaugherty"), sanitized.
  String get username =>
      _username.value ??
      _suggestedUsername(realName).truncate(kMaxUsernameLength);

  static String _suggestedUsername(String realName) {
    final words =
        realName.trim().split(RegExp(r'\\s+')).where((w) => w.isNotEmpty);
    final name = words.length < 2 ? realName : words.first[0] + words.last;
    return name.sanitize().toLowerCase();
  }''')
print('username: first initial and last name')

# Patrone's licence agreement, after "Try or install" (patrone_licence_page.dart)
page = bootstrap / 'lib/pages/patrone_licence/patrone_licence_page.dart'
page.parent.mkdir(exist_ok=True)
shutil.copyfile(Path(__file__).resolve().parent / 'patrone_licence_page.dart', page)
edit(bootstrap / 'lib/pages.dart',
     "export 'pages/loading/loading_page.dart';\n",
     "export 'pages/loading/loading_page.dart';\n"
     "export 'pages/patrone_licence/patrone_licence_page.dart';\n")
edit(bootstrap / 'lib/app/installation_step.dart',
     '  tryOrInstall(TryOrInstallPage.new, discreteStep: false, allowedToHide: true),\n',
     '  tryOrInstall(TryOrInstallPage.new, discreteStep: false, allowedToHide: true),\n'
     '  patroneLicence(PatroneLicencePage.new, allowedToHide: true),\n')
print('licence agreement page')
