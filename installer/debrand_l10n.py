#!/usr/bin/env python3
# Takes "Ubuntu" out of the installer's text, for installer/build.sh.
#
#   debrand_l10n.py L10N-DIR...
#
# Each L10N-DIR has the app's translations as ARB files, English
# (*_en.arb) the one the others follow. Nearly all of the installer's text
# names the system as {DISTRO}, the flavor's name; this changes the English
# messages that say "Ubuntu" themselves to say "Patrone Linux" and drops
# their translations, which then show the English. A translation that says
# "Ubuntu" where the English doesn't is dropped too. Ubuntu Pro's pages,
# Canonical's product, keep their text: Patrone's installer doesn't show
# them.
import json
import sys
from pathlib import Path


def is_pro(key, value):
    return key.lower().startswith('ubuntupro') or 'Ubuntu Pro' in value


for directory in map(Path, sys.argv[1:]):
    (english_path,) = directory.glob('*_en.arb')
    english = json.loads(english_path.read_text(encoding='utf-8'))
    changed = set()
    for key, value in english.items():
        if key.startswith('@') or not isinstance(value, str):
            continue
        if 'Ubuntu' in value and not is_pro(key, value):
            english[key] = value.replace('Ubuntu Desktop Installer', 'Patrone Linux Installer') \
                .replace('Ubuntu', 'Patrone Linux')
            changed.add(key)
    english_path.write_text(json.dumps(english, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f'{english_path}: {len(changed)} messages')

    for path in sorted(directory.glob('*.arb')):
        if path == english_path:
            continue
        messages = json.loads(path.read_text(encoding='utf-8'))
        dropped = [key for key, value in messages.items()
                   if not key.startswith('@') and isinstance(value, str)
                   and (key in changed
                        or ('Ubuntu' in value and 'Ubuntu' not in english.get(key, '')))]
        for key in dropped:
            del messages[key]
            messages.pop('@' + key, None)
        if dropped:
            path.write_text(json.dumps(messages, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
