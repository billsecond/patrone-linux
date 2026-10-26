#!/usr/bin/env python3
"""Draws Patrone File Manager's color icons as SVG files.

    python3 tools/make_icons.py data/icons

Every icon is original artwork in the spirit of Windows 11's Fluent style:
soft gradients, rounded corners and a consistent light source. The large
icons are drawn on a 64-unit grid and scale to every view size; the
navigation pane gets its own simpler drawings on a 16-unit grid, which stay
readable at 16 pixels. Nothing here is copied from Microsoft's icons.
"""

import os
import sys

# Patrone's palette (branding/README.md) plus the colors each kind uses.
BLUE = "#288BFA"
BLUE_HI = "#4DA0FC"
BLUE_MID = "#2372DD"
BLUE_DEEP = "#1851A8"
DOC_BLUE = "#2B6FD6"
GREEN = "#1E9E57"
ORANGE = "#F07A22"
RED = "#D93A3A"
PURPLE = "#8A4FE8"
SLATE = "#3C4A5C"
TEAL = "#0E8FC9"
INDIGO = "#5B5FC7"
BROWN = "#A0662E"


def svg(size, body, defs=""):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" '
        f'viewBox="0 0 {size} {size}">'
        + (f"<defs>{defs}</defs>" if defs else "")
        + body
        + "</svg>\n"
    )


def gradient(name, top, bottom, x2=0, y2=1):
    return (
        f'<linearGradient id="{name}" x1="0" y1="0" x2="{x2}" y2="{y2}">'
        f'<stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/>'
        "</linearGradient>"
    )


# --- Folders (64 grid) -------------------------------------------------------

FOLDER_DEFS = (
    gradient("back", "#EAA51E", "#D38A0B")
    + gradient("front", "#FFD96B", "#FDBE36")
)
FOLDER_BACK = (
    '<path d="M6 14.5A4.5 4.5 0 0 1 10.5 10H23.4a4.5 4.5 0 0 1 3.3 1.4L30.4 15'
    'H53.5A4.5 4.5 0 0 1 58 19.5V49.5A4.5 4.5 0 0 1 53.5 54H10.5A4.5 4.5 0 0 1 6 49.5Z" '
    'fill="url(#back)"/>'
)
FOLDER_PAPER = '<rect x="10.5" y="16" width="43" height="14" rx="2" fill="#FFFFFF" opacity=".92"/>'
FOLDER_FRONT = (
    '<path d="M6 25A4.5 4.5 0 0 1 10.5 20.5H53.5A4.5 4.5 0 0 1 58 25V49.5'
    'A4.5 4.5 0 0 1 53.5 54H10.5A4.5 4.5 0 0 1 6 49.5Z" fill="url(#front)"/>'
    '<path d="M10.5 21.25H53.5" stroke="#FFEBB0" stroke-width="1.5" stroke-linecap="round"/>'
)


def folder(emblem="", defs="", paper=True):
    body = FOLDER_BACK + (FOLDER_PAPER if paper else "") + FOLDER_FRONT + emblem
    return svg(64, body, FOLDER_DEFS + defs)


def emblem_documents():
    return (
        '<rect x="25" y="28.5" width="14" height="18" rx="1.6" fill="#FFFFFF" '
        f'stroke="{DOC_BLUE}" stroke-width="1.5"/>'
        f'<path d="M28 33.5H36M28 37H36M28 40.5H33" stroke="{DOC_BLUE}" '
        'stroke-width="1.5" stroke-linecap="round"/>'
    )


def emblem_downloads():
    return (
        f'<path d="M29.2 28.5H34.8V36.5H39.2L32 44L24.8 36.5H29.2Z" fill="{GREEN}" '
        f'stroke="{GREEN}" stroke-width="1" stroke-linejoin="round"/>'
        f'<path d="M24.5 47.5H39.5" stroke="{GREEN}" stroke-width="2.4" stroke-linecap="round"/>'
    )


def emblem_pictures():
    return (
        f'<rect x="22.5" y="29" width="19" height="16" rx="2" fill="{BLUE}"/>'
        '<path d="M22.5 41.5L28.5 35.5L33 40L35.5 37.5L41.5 43V43A2 2 0 0 1 39.5 45H24.5'
        'A2 2 0 0 1 22.5 43Z" fill="#BFE1FF"/>'
        '<circle cx="36.5" cy="33.5" r="2" fill="#FFF0A8"/>'
    )


def emblem_music():
    return (
        f'<path d="M28.5 32.5L38 30V42" stroke="{ORANGE}" stroke-width="2" fill="none" '
        'stroke-linejoin="round"/>'
        f'<path d="M28.5 32.5V44" stroke="{ORANGE}" stroke-width="2"/>'
        f'<path d="M28.5 30.7L38 28.2V31.8L28.5 34.3Z" fill="{ORANGE}"/>'
        f'<ellipse cx="26" cy="44.2" rx="3" ry="2.4" fill="{ORANGE}"/>'
        f'<ellipse cx="35.5" cy="42.2" rx="3" ry="2.4" fill="{ORANGE}"/>'
    )


def emblem_videos():
    return (
        f'<rect x="22.5" y="29.5" width="19" height="15" rx="2.2" fill="{PURPLE}"/>'
        '<path d="M29.5 33.4V40.6L35.6 37Z" fill="#FFFFFF"/>'
    )


def emblem_desktop():
    return (
        f'<rect x="22" y="29" width="20" height="13.5" rx="1.6" fill="{BLUE}"/>'
        '<rect x="23.5" y="30.5" width="17" height="10.5" rx=".8" fill="#9CCBFF"/>'
        f'<path d="M32 42.5V46M27.5 46.5H36.5" stroke="{SLATE}" stroke-width="1.6" '
        'stroke-linecap="round"/>'
    )


def emblem_templates():
    return (
        '<rect x="24.5" y="28.5" width="15" height="18" rx="1.6" fill="#FFFFFF" '
        'stroke="#6B7A90" stroke-width="1.5"/>'
        '<rect x="27.5" y="31.5" width="9" height="4" rx=".8" fill="#9AA7B8"/>'
        '<path d="M27.5 39H36.5M27.5 42.5H33" stroke="#9AA7B8" stroke-width="1.5" '
        'stroke-linecap="round"/>'
    )


def emblem_public():
    return (
        f'<circle cx="28" cy="33" r="3" fill="{TEAL}"/>'
        f'<path d="M22 45A6 6 0 0 1 34 45Z" fill="{TEAL}"/>'
        '<circle cx="36.5" cy="34" r="2.6" fill="#45B7E6"/>'
        '<path d="M31.5 45A5 5 0 0 1 41.5 45Z" fill="#45B7E6"/>'
    )


def emblem_user():
    return (
        f'<circle cx="32" cy="33" r="4" fill="{BLUE_MID}"/>'
        f'<path d="M24 46.5A8 8 0 0 1 40 46.5Z" fill="{BLUE_MID}"/>'
    )


def folder_zip():
    zipper = (
        '<rect x="28.5" y="20.5" width="7" height="33.5" fill="#5E6670"/>'
        + "".join(
            f'<rect x="{29.5 if i % 2 == 0 else 32}" y="{22 + i * 3}" width="2.5" height="1.6" '
            'rx=".5" fill="#D9DEE4"/>'
            for i in range(8)
        )
        + '<rect x="27.5" y="45" width="9" height="7" rx="1.5" fill="#3F464F"/>'
        '<rect x="30" y="47" width="4" height="3" rx=".8" fill="#D9DEE4"/>'
    )
    return folder(zipper, paper=False)


def app_icon():
    """A folder carrying the slanted bar of the Patrone mark."""
    defs = gradient("pmark", BLUE_HI, BLUE_DEEP, x2=0.35, y2=1)
    mark = (
        '<path d="M27.5 25.5H39L31.5 50.5H20Z" fill="url(#pmark)"/>'
        f'<path d="M38.5 25.5H47.5L44.4 36H35.4Z" fill="{BLUE_HI}"/>'
    )
    return folder(mark, defs)


# --- Files (64 grid) ---------------------------------------------------------

PAGE = (
    '<path d="M15 8.5A2.5 2.5 0 0 1 17.5 6H39.5L51 17.5V55.5A2.5 2.5 0 0 1 48.5 58'
    'H17.5A2.5 2.5 0 0 1 15 55.5Z" fill="#FFFFFF" stroke="#B4BAC2" stroke-width="1.4" '
    'stroke-linejoin="round"/>'
    '<path d="M39.5 6V15A2.5 2.5 0 0 0 42 17.5H51Z" fill="#E4E8ED" stroke="#B4BAC2" '
    'stroke-width="1.4" stroke-linejoin="round"/>'
)


def page(content="", defs=""):
    return svg(64, PAGE + content, defs)


def lines(color, rows, x1=21, x2=45, short=None):
    out = ""
    for i, y in enumerate(rows):
        end = short if short is not None and i == len(rows) - 1 else x2
        out += (
            f'<path d="M{x1} {y}H{end}" stroke="{color}" stroke-width="2" '
            'stroke-linecap="round"/>'
        )
    return out


def badge(color, symbol):
    """A colored square at the page's lower left with a white symbol."""
    return (
        f'<rect x="9" y="33" width="24" height="24" rx="3.5" fill="{color}"/>'
        '<path d="M9 36.5A3.5 3.5 0 0 1 12.5 33H29.5A3.5 3.5 0 0 1 33 36.5V38H9Z" '
        'fill="#FFFFFF" opacity=".14"/>'
        + symbol
    )


WHITE = 'stroke="#FFFFFF" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"'
PAGE_LINES = lines("#C9CED6", [33, 38, 43, 48], x1=37, x2=45)


def file_text():
    return page(lines("#A3AAB4", [24, 29, 34, 39, 44, 49], short=36))


def file_document():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(DOC_BLUE, f'<path d="M14 40H28M14 45H28M14 50H23" {WHITE}/>')
    )


def file_spreadsheet():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(GREEN, f'<path d="M14 39.5H28V51.5H14ZM14 45.5H28M21 39.5V51.5" {WHITE}/>')
    )


def file_presentation():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge("#D9532B", f'<path d="M14.5 51.5V46.5M19 51.5V41.5M23.5 51.5V44M28 51.5V39" {WHITE}/>')
    )


def file_pdf():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(RED, '<path d="M16 38.5H26V52L21 48L16 52Z" fill="#FFFFFF"/>')
    )


def file_image():
    return page(
        f'<rect x="20" y="24" width="26" height="22" rx="2.5" fill="{BLUE}"/>'
        '<path d="M20 41L27.5 33.5L33.5 39.5L37 36L46 44.5A2.5 2.5 0 0 1 43.5 46H22.5'
        'A2.5 2.5 0 0 1 20 43.5Z" fill="#BFE1FF"/>'
        '<path d="M20 43.5L27.5 36.5L35 43.5L37.5 41L42 46H22.5A2.5 2.5 0 0 1 20 43.5Z" '
        'fill="#7CC48A"/>'
        '<circle cx="39.5" cy="30" r="2.8" fill="#FFF0A8"/>'
    )


def file_audio():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(
            ORANGE,
            f'<path d="M18 49V39.5L27 37.5V47" {WHITE}/>'
            '<ellipse cx="16" cy="49.2" rx="2.4" ry="2" fill="#FFFFFF"/>'
            '<ellipse cx="25" cy="47.2" rx="2.4" ry="2" fill="#FFFFFF"/>',
        )
    )


def file_video():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(PURPLE, '<path d="M17.5 39.5V50.5L27 45Z" fill="#FFFFFF"/>')
    )


def file_code():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(INDIGO, f'<path d="M17 40.5L13.5 45L17 49.5M25 40.5L28.5 45L25 49.5M22.5 39L19.5 51" {WHITE}/>')
    )


def file_script():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(SLATE, f'<path d="M14 40L18.5 44.5L14 49M21 50H28" {WHITE}/>')
    )


def file_font():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge("#7D5BA6", f'<path d="M14.5 51L21 38.5L27.5 51M17 46.5H25" {WHITE}/>')
    )


def file_package():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(
            BROWN,
            f'<path d="M21 38.5L28 42V49.5L21 53L14 49.5V42ZM14 42L21 45.5L28 42M21 45.5V53" {WHITE}/>',
        )
    )


def file_web():
    return page(
        lines("#C9CED6", [22, 27], x1=21, x2=45)
        + PAGE_LINES
        + badge(
            TEAL,
            f'<circle cx="21" cy="45" r="6.5" {WHITE}/>'
            f'<path d="M14.5 45H27.5M21 38.5C18.6 41 18.6 49 21 51.5M21 38.5C23.4 41 23.4 49 21 51.5" {WHITE}/>',
        )
    )


def file_disc():
    defs = gradient("disc", "#E9EDF2", "#AEB8C4", x2=1, y2=1)
    return page(
        '<circle cx="33" cy="38" r="13" fill="url(#disc)" stroke="#9AA5B1" stroke-width="1"/>'
        '<path d="M24 31A13 13 0 0 1 33 25" stroke="#FFFFFF" stroke-width="2" fill="none" '
        'stroke-linecap="round" opacity=".9"/>'
        '<circle cx="33" cy="38" r="3.5" fill="#FFFFFF" stroke="#9AA5B1" stroke-width="1"/>',
        defs,
    )


# --- Drives, places and the Recycle Bin (64 grid) -----------------------------

DRIVE_DEFS = gradient("body", "#E3E7EC", "#B3BCC6") + gradient("strip", "#6A7683", "#4D5763")


def drive_body(extra=""):
    return (
        '<rect x="7" y="22" width="50" height="24" rx="5" fill="url(#body)" '
        'stroke="#8F9AA6" stroke-width="1"/>'
        '<path d="M7 37H57V41A5 5 0 0 1 52 46H12A5 5 0 0 1 7 41Z" fill="url(#strip)"/>'
        '<path d="M12 23H52" stroke="#FFFFFF" stroke-width="1.2" stroke-linecap="round" opacity=".8"/>'
        '<circle cx="50.5" cy="41.5" r="1.8" fill="#43D17E"/>'
        + extra
    )


def drive():
    return svg(64, drive_body(), DRIVE_DEFS)


def drive_system():
    mark = (
        '<path d="M14.5 26.5H20.5L18 34H12Z" fill="url(#pmark)"/>'
        '<path d="M19.5 26.5H23.5L22.2 30.5H18.2Z" fill="' + BLUE_HI + '"/>'
    )
    return svg(64, drive_body(mark), DRIVE_DEFS + gradient("pmark", BLUE_HI, BLUE_DEEP))


def drive_usb():
    defs = gradient("stick", "#5A6675", "#3A434F") + gradient("plug", "#E9EDF2", "#B5BFCA")
    return svg(
        64,
        '<rect x="24" y="4" width="16" height="13" rx="1.5" fill="url(#plug)" stroke="#8F9AA6"/>'
        '<rect x="27.5" y="8" width="3" height="3" fill="#6A7683"/>'
        '<rect x="33.5" y="8" width="3" height="3" fill="#6A7683"/>'
        '<rect x="19" y="16" width="26" height="44" rx="5" fill="url(#stick)"/>'
        f'<rect x="19" y="42" width="26" height="5" fill="{BLUE}"/>'
        '<path d="M23 19H41" stroke="#FFFFFF" stroke-width="1.2" stroke-linecap="round" opacity=".35"/>',
        defs,
    )


def drive_optical():
    defs = (
        '<radialGradient id="disc" cx=".35" cy=".3" r=".8">'
        '<stop offset="0" stop-color="#FFFFFF"/><stop offset=".45" stop-color="#D5E6F7"/>'
        '<stop offset=".7" stop-color="#E8D7F2"/><stop offset="1" stop-color="#B8C3CF"/>'
        "</radialGradient>"
    )
    return svg(
        64,
        '<circle cx="32" cy="32" r="26" fill="url(#disc)" stroke="#8F9AA6" stroke-width="1"/>'
        '<circle cx="32" cy="32" r="8" fill="#DDE3EA" stroke="#8F9AA6" stroke-width="1"/>'
        '<circle cx="32" cy="32" r="3.5" fill="#FFFFFF" stroke="#8F9AA6" stroke-width="1"/>',
        defs,
    )


def drive_network():
    return svg(
        64,
        drive_body().replace('y="22"', 'y="14"').replace("M7 37H57V41A5 5 0 0 1 52 46H12A5 5 0 0 1 7 41Z", "M7 29H57V33A5 5 0 0 1 52 38H12A5 5 0 0 1 7 33Z").replace('cy="41.5"', 'cy="33.5"').replace("M12 23H52", "M12 15H52")
        + f'<path d="M32 38V50M14 52H50" stroke="{SLATE}" stroke-width="2.4" stroke-linecap="round"/>'
        f'<rect x="27" y="47" width="10" height="9" rx="2" fill="{BLUE}"/>',
        DRIVE_DEFS,
    )


def computer():
    defs = gradient("screen", BLUE_HI, BLUE_DEEP, x2=0.6, y2=1) + gradient("base", "#C9D0D8", "#8F9AA6")
    return svg(
        64,
        '<rect x="5" y="9" width="54" height="37" rx="4" fill="#2E3640"/>'
        '<rect x="8" y="12" width="48" height="31" rx="1.5" fill="url(#screen)"/>'
        '<path d="M8 36C20 30 33 33 56 22V41.5A1.5 1.5 0 0 1 54.5 43H9.5A1.5 1.5 0 0 1 8 41.5Z" '
        'fill="#FFFFFF" opacity=".12"/>'
        '<path d="M27 46H37L38.5 53H25.5Z" fill="url(#base)"/>'
        '<rect x="18" y="52.5" width="28" height="4.5" rx="2" fill="url(#base)"/>',
        defs,
    )


def network():
    defs = gradient("globe", BLUE_HI, BLUE_DEEP, x2=0.5, y2=1)
    return svg(
        64,
        '<circle cx="32" cy="28" r="21" fill="url(#globe)"/>'
        '<path d="M11 28H53M32 7C25 13 25 43 32 49M32 7C39 13 39 43 32 49M14.5 17H49.5M14.5 39H49.5" '
        'stroke="#FFFFFF" stroke-width="1.8" fill="none" opacity=".75"/>'
        f'<path d="M32 49V55M18 57H46" stroke="{SLATE}" stroke-width="2.6" stroke-linecap="round"/>',
        defs,
    )


TRASH_DEFS = gradient("bin", "#EEF3F8", "#C2CEDB") + gradient("lid", "#B9C5D2", "#8E9DAE")


def trash(full=False):
    papers = ""
    if full:
        papers = (
            '<path d="M20 22L23 8L37 11L34 25Z" fill="#FFFFFF" stroke="#B4BAC2"/>'
            f'<path d="M30 23L36 6.5L48 11L42 27Z" fill="#DDEBFF" stroke="{BLUE}"/>'
            '<path d="M24 12.5L33 14.5M23 16.5L32 18.5" stroke="#C9CED6" stroke-width="1.5" '
            'stroke-linecap="round"/>'
        )
    return svg(
        64,
        papers
        + '<path d="M15 21H49L46 54.5A3.5 3.5 0 0 1 42.5 58H21.5A3.5 3.5 0 0 1 18 54.5Z" '
        'fill="url(#bin)" stroke="#8695A6" stroke-width="1.2" stroke-linejoin="round"/>'
        '<path d="M25 27L26 51M32 27V51M39 27L38 51" stroke="#9AA8B8" stroke-width="2" '
        'stroke-linecap="round"/>'
        '<rect x="11" y="16" width="42" height="7" rx="2.5" fill="url(#lid)"/>'
        '<path d="M26 16V13.5A1.5 1.5 0 0 1 27.5 12H36.5A1.5 1.5 0 0 1 38 13.5V16" '
        'stroke="#8E9DAE" stroke-width="2" fill="none"/>',
        TRASH_DEFS,
    )


def home():
    defs = gradient("wall", BLUE_HI, BLUE_MID) + gradient("roof", "#3E7FE0", BLUE_DEEP)
    return svg(
        64,
        '<path d="M13 30V54A3 3 0 0 0 16 57H48A3 3 0 0 0 51 54V30L32 14Z" fill="url(#wall)"/>'
        '<path d="M7 32L32 10L57 32" stroke="url(#roof)" stroke-width="6" fill="none" '
        'stroke-linecap="round" stroke-linejoin="round"/>'
        '<rect x="27" y="39" width="10" height="18" rx="1.5" fill="#FFFFFF" opacity=".92"/>',
        defs,
    )


def gallery():
    return svg(
        64,
        '<rect x="14" y="8" width="38" height="30" rx="3" fill="#FFFFFF" stroke="#B4BAC2" '
        'transform="rotate(8 33 23)"/>'
        f'<rect x="8" y="18" width="42" height="34" rx="3.5" fill="{BLUE}"/>'
        '<path d="M8 44L20 32L29 41L35 35L50 49V48.5A3.5 3.5 0 0 1 46.5 52H11.5A3.5 3.5 0 0 1 8 48.5Z" '
        'fill="#BFE1FF"/>'
        '<path d="M8 48L20 37L33 49L37 46L41 52H11.5A3.5 3.5 0 0 1 8 48.5Z" fill="#7CC48A"/>'
        '<circle cx="40" cy="27" r="3.5" fill="#FFF0A8"/>',
    )


def star():
    defs = gradient("star", "#FFD45C", "#F2A900")
    return svg(
        64,
        '<path d="M32 6L39.6 21.9L57 24.2L44.3 36.3L47.5 53.6L32 45.2L16.5 53.6L19.7 36.3'
        'L7 24.2L24.4 21.9Z" fill="url(#star)" stroke="#E09A00" stroke-width="1.2" '
        'stroke-linejoin="round"/>',
        defs,
    )


# --- Navigation pane icons (16 grid) -----------------------------------------


def nav(body, defs=""):
    return svg(16, body, defs)


NAV_FOLDER_DEFS = gradient("nb", "#EAA51E", "#D38A0B") + gradient("nf", "#FFD96B", "#FDBE36")
NAV_FOLDER = (
    '<path d="M1 3.6A1.1 1.1 0 0 1 2.1 2.5H5.6L7 3.9H13.9A1.1 1.1 0 0 1 15 5V12.9'
    'A1.1 1.1 0 0 1 13.9 14H2.1A1.1 1.1 0 0 1 1 12.9Z" fill="url(#nb)"/>'
    '<path d="M1 6.1A1.1 1.1 0 0 1 2.1 5H13.9A1.1 1.1 0 0 1 15 6.1V12.9A1.1 1.1 0 0 1 13.9 14'
    'H2.1A1.1 1.1 0 0 1 1 12.9Z" fill="url(#nf)"/>'
)


def nav_folder():
    return nav(NAV_FOLDER, NAV_FOLDER_DEFS)


def nav_home():
    return nav(
        f'<path d="M3 7.5V13.5A.5.5 0 0 0 3.5 14H12.5A.5.5 0 0 0 13 13.5V7.5L8 3.5Z" fill="{BLUE}"/>'
        f'<path d="M1.5 8L8 2.5L14.5 8" stroke="{BLUE_DEEP}" stroke-width="1.6" fill="none" '
        'stroke-linecap="round" stroke-linejoin="round"/>'
        '<rect x="6.6" y="9.5" width="2.8" height="4.5" fill="#FFFFFF"/>'
    )


def nav_gallery():
    return nav(
        '<rect x="3.5" y="1.5" width="11" height="8.5" rx="1" fill="#FFFFFF" stroke="#9AA3AD" '
        'stroke-width=".8"/>'
        f'<rect x="1" y="4.5" width="11.5" height="10" rx="1.2" fill="{BLUE}"/>'
        '<path d="M1 12.3L4.5 9L7 11.5L8.5 10L12.5 13.8V13.3A1.2 1.2 0 0 1 11.3 14.5H2.2'
        'A1.2 1.2 0 0 1 1 13.3Z" fill="#7CC48A"/>'
        '<circle cx="9.3" cy="7.3" r="1.1" fill="#FFF0A8"/>'
    )


def nav_user():
    return nav(
        f'<circle cx="8" cy="5" r="3" fill="{BLUE}"/>'
        f'<path d="M2 14.5A6 6 0 0 1 14 14.5Z" fill="{BLUE_MID}"/>'
    )


def nav_desktop():
    return nav(
        f'<rect x="1" y="2" width="14" height="9.5" rx="1.2" fill="{BLUE}"/>'
        '<rect x="2.2" y="3.2" width="11.6" height="7.1" rx=".5" fill="#9CCBFF"/>'
        f'<path d="M8 11.5V13.5M5 14.2H11" stroke="{SLATE}" stroke-width="1.4" stroke-linecap="round"/>'
    )


def nav_downloads():
    return nav(
        f'<path d="M6.3 1.5H9.7V7H12.5L8 11.7L3.5 7H6.3Z" fill="{GREEN}" stroke="{GREEN}" '
        'stroke-width=".6" stroke-linejoin="round"/>'
        f'<path d="M2.5 14H13.5" stroke="{GREEN}" stroke-width="1.8" stroke-linecap="round"/>'
    )


def nav_documents():
    return nav(
        '<path d="M3 1.8A.8.8 0 0 1 3.8 1H10L13 4V14.2A.8.8 0 0 1 12.2 15H3.8A.8.8 0 0 1 3 14.2Z" '
        'fill="#FFFFFF" stroke="#8C96A2" stroke-width=".9"/>'
        f'<path d="M5.5 6.5H10.5M5.5 9H10.5M5.5 11.5H8.5" stroke="{DOC_BLUE}" stroke-width="1.2" '
        'stroke-linecap="round"/>'
    )


def nav_pictures():
    return nav(
        f'<rect x="1" y="2.5" width="14" height="11" rx="1.3" fill="{BLUE}"/>'
        '<path d="M1 11L5 7L8 10L10 8L15 12.5V12.2A1.3 1.3 0 0 1 13.7 13.5H2.3A1.3 1.3 0 0 1 1 12.2Z" '
        'fill="#BFE1FF"/>'
        '<circle cx="11" cy="5.5" r="1.3" fill="#FFF0A8"/>'
    )


def nav_music():
    return nav(
        f'<path d="M5.5 4V12.5M12.5 2.5V11" stroke="{ORANGE}" stroke-width="1.4"/>'
        f'<path d="M5.5 3.5L12.5 1.5V4L5.5 6Z" fill="{ORANGE}"/>'
        f'<ellipse cx="3.6" cy="12.6" rx="2.1" ry="1.7" fill="{ORANGE}"/>'
        f'<ellipse cx="10.6" cy="11.1" rx="2.1" ry="1.7" fill="{ORANGE}"/>'
    )


def nav_videos():
    return nav(
        f'<rect x="1" y="2.5" width="14" height="11" rx="1.8" fill="{PURPLE}"/>'
        '<path d="M6 5.3V10.7L10.6 8Z" fill="#FFFFFF"/>'
    )


def nav_templates():
    return nav(
        '<rect x="2.5" y="1" width="11" height="14" rx="1" fill="#FFFFFF" stroke="#6B7A90" '
        'stroke-width=".9"/>'
        '<rect x="4.5" y="3" width="7" height="3.5" rx=".5" fill="#9AA7B8"/>'
        '<path d="M4.5 9H11.5M4.5 11.5H9" stroke="#9AA7B8" stroke-width="1.1" stroke-linecap="round"/>'
    )


def nav_public():
    return nav(
        f'<circle cx="5.5" cy="5" r="2.4" fill="{TEAL}"/>'
        f'<path d="M1 14A4.5 4.5 0 0 1 10 14Z" fill="{TEAL}"/>'
        '<circle cx="11.5" cy="6" r="2" fill="#45B7E6"/>'
        '<path d="M8 14A3.8 3.8 0 0 1 15 14Z" fill="#45B7E6"/>'
    )


def nav_computer():
    return nav(
        '<rect x="1" y="2" width="14" height="9.5" rx="1.2" fill="#2E3640"/>'
        '<rect x="2" y="3" width="12" height="7.5" rx=".4" fill="url(#ns)"/>'
        '<path d="M6.5 11.5H9.5L10 13.5H6Z" fill="#8F9AA6"/>'
        '<rect x="4" y="13.3" width="8" height="1.6" rx=".6" fill="#8F9AA6"/>',
        gradient("ns", BLUE_HI, BLUE_DEEP, x2=0.6, y2=1),
    )


NAV_DRIVE_DEFS = gradient("db", "#E3E7EC", "#B3BCC6")


def nav_drive(extra=""):
    return nav(
        '<rect x="1" y="4.5" width="14" height="7.5" rx="1.6" fill="url(#db)" stroke="#8F9AA6" '
        'stroke-width=".7"/>'
        '<path d="M1 9H15V10.4A1.6 1.6 0 0 1 13.4 12H2.6A1.6 1.6 0 0 1 1 10.4Z" fill="#56616D"/>'
        '<circle cx="12.8" cy="10.5" r=".7" fill="#43D17E"/>'
        + extra,
        NAV_DRIVE_DEFS,
    )


def nav_drive_system():
    return nav_drive(
        f'<path d="M3.2 5.6H5.4L4.6 8.2H2.4Z" fill="{BLUE}"/>'
        f'<path d="M5 5.6H6.6L6.2 7H4.6Z" fill="{BLUE_HI}"/>'
    )


def nav_drive_usb():
    return nav(
        '<rect x="5.5" y="1" width="5" height="3.5" rx=".5" fill="#C9D0D8" stroke="#8F9AA6" '
        'stroke-width=".6"/>'
        '<rect x="4" y="4" width="8" height="11" rx="1.5" fill="#46505C"/>'
        f'<rect x="4" y="10" width="8" height="1.6" fill="{BLUE}"/>'
    )


def nav_drive_optical():
    return nav(
        '<circle cx="8" cy="8" r="7" fill="#DCE6F0" stroke="#8F9AA6" stroke-width=".7"/>'
        '<path d="M3.5 5.5A5 5 0 0 1 7 3" stroke="#FFFFFF" stroke-width="1.2" fill="none" '
        'stroke-linecap="round"/>'
        '<circle cx="8" cy="8" r="2" fill="#FFFFFF" stroke="#8F9AA6" stroke-width=".7"/>'
    )


def nav_drive_network():
    return nav(
        '<rect x="1" y="1.5" width="14" height="7" rx="1.6" fill="url(#db)" stroke="#8F9AA6" '
        'stroke-width=".7"/>'
        '<path d="M1 5.5H15V6.9A1.6 1.6 0 0 1 13.4 8.5H2.6A1.6 1.6 0 0 1 1 6.9Z" fill="#56616D"/>'
        f'<path d="M8 8.5V12.5M2.5 14H13.5" stroke="{SLATE}" stroke-width="1.3" stroke-linecap="round"/>'
        f'<rect x="6.3" y="11.5" width="3.4" height="3" rx=".6" fill="{BLUE}"/>',
        NAV_DRIVE_DEFS,
    )


def nav_network():
    return nav(
        f'<circle cx="8" cy="7" r="6" fill="{BLUE}"/>'
        '<path d="M2 7H14M8 1C6 3 6 11 8 13M8 1C10 3 10 11 8 13" stroke="#FFFFFF" '
        'stroke-width=".9" fill="none" opacity=".8"/>'
        f'<path d="M8 13V14.5M4 15H12" stroke="{SLATE}" stroke-width="1.1" stroke-linecap="round"/>'
    )


def nav_trash(full=False):
    papers = ""
    if full:
        papers = (
            '<path d="M5 5L5.8 1L9.5 1.8L8.7 5.8Z" fill="#FFFFFF" stroke="#8C96A2" stroke-width=".6"/>'
            f'<path d="M7.6 5.5L9.4 1.3L12.6 2.6L10.8 6.8Z" fill="#DDEBFF" stroke="{BLUE}" stroke-width=".6"/>'
        )
    return nav(
        papers
        + '<path d="M3 5.5H13L12.2 14A1.1 1.1 0 0 1 11.1 15H4.9A1.1 1.1 0 0 1 3.8 14Z" '
        'fill="#E6EDF4" stroke="#7D8C9D" stroke-width=".9" stroke-linejoin="round"/>'
        '<path d="M6.2 7.5V13M9.8 7.5V13" stroke="#9AA8B8" stroke-width="1" stroke-linecap="round"/>'
        '<rect x="2" y="4" width="12" height="2.2" rx=".8" fill="#8E9DAE"/>'
    )


ICONS = {
    "pfm-folder": lambda: folder(),
    "pfm-folder-documents": lambda: folder(emblem_documents()),
    "pfm-folder-downloads": lambda: folder(emblem_downloads()),
    "pfm-folder-pictures": lambda: folder(emblem_pictures()),
    "pfm-folder-music": lambda: folder(emblem_music()),
    "pfm-folder-videos": lambda: folder(emblem_videos()),
    "pfm-folder-desktop": lambda: folder(emblem_desktop()),
    "pfm-folder-templates": lambda: folder(emblem_templates()),
    "pfm-folder-public": lambda: folder(emblem_public()),
    "pfm-folder-user": lambda: folder(emblem_user()),
    "pfm-folder-zip": folder_zip,
    "pfm-file": lambda: page(),
    "pfm-file-text": file_text,
    "pfm-file-document": file_document,
    "pfm-file-spreadsheet": file_spreadsheet,
    "pfm-file-presentation": file_presentation,
    "pfm-file-pdf": file_pdf,
    "pfm-file-image": file_image,
    "pfm-file-audio": file_audio,
    "pfm-file-video": file_video,
    "pfm-file-code": file_code,
    "pfm-file-script": file_script,
    "pfm-file-font": file_font,
    "pfm-file-package": file_package,
    "pfm-file-web": file_web,
    "pfm-file-disc": file_disc,
    "pfm-drive": drive,
    "pfm-drive-system": drive_system,
    "pfm-drive-usb": drive_usb,
    "pfm-drive-optical": drive_optical,
    "pfm-drive-network": drive_network,
    "pfm-computer": computer,
    "pfm-network": network,
    "pfm-trash": lambda: trash(False),
    "pfm-trash-full": lambda: trash(True),
    "pfm-home": home,
    "pfm-gallery": gallery,
    "pfm-star": star,
    "pfm-nav-folder": nav_folder,
    "pfm-nav-home": nav_home,
    "pfm-nav-gallery": nav_gallery,
    "pfm-nav-user": nav_user,
    "pfm-nav-desktop": nav_desktop,
    "pfm-nav-downloads": nav_downloads,
    "pfm-nav-documents": nav_documents,
    "pfm-nav-pictures": nav_pictures,
    "pfm-nav-music": nav_music,
    "pfm-nav-videos": nav_videos,
    "pfm-nav-templates": nav_templates,
    "pfm-nav-public": nav_public,
    "pfm-nav-computer": nav_computer,
    "pfm-nav-drive": nav_drive,
    "pfm-nav-drive-system": nav_drive_system,
    "pfm-nav-drive-usb": nav_drive_usb,
    "pfm-nav-drive-optical": nav_drive_optical,
    "pfm-nav-drive-network": nav_drive_network,
    "pfm-nav-network": nav_network,
    "pfm-nav-trash": lambda: nav_trash(False),
    "pfm-nav-trash-full": lambda: nav_trash(True),
}


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "data/icons"
    os.makedirs(out, exist_ok=True)
    for name, draw in ICONS.items():
        with open(os.path.join(out, name + ".svg"), "w", encoding="utf-8") as file:
            file.write(draw())
    # The app's own icon also goes into the system icon theme.
    with open(os.path.join(out, "..", "org.patrone.FileManager.svg"), "w", encoding="utf-8") as file:
        file.write(app_icon())
    print(f"wrote {len(ICONS) + 1} icons")


if __name__ == "__main__":
    main()
