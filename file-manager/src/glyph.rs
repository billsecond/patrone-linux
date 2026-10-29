//! Patrone's line icons for the command bar, menus and buttons.
//!
//! Each glyph is a handful of SVG path strings on a 16-unit grid, drawn with
//! 1-unit round strokes in the widget's CSS color, so glyphs follow light and
//! dark mode, hover and disabled states, and stay sharp at any scale. Parts
//! marked as accent are drawn in Patrone Blue. They're original artwork
//! drawn for Patrone.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, prelude::*};

/// How one part of a glyph is painted.
#[derive(Clone, Copy)]
enum Paint {
    Stroke,
    Fill,
    AccentStroke,
    AccentFill,
    /// A fixed color, such as the gold of a favorite's star.
    ColorFill(u32),
    ColorStroke(u32),
}

use Paint::*;

const CIRCLE_15: &str = "M14.5 8A6.5 6.5 0 1 1 1.5 8A6.5 6.5 0 1 1 14.5 8Z";
const FOLDER: &str = "M1.5 4A1.5 1.5 0 0 1 3 2.5H6.1L7.6 4H13A1.5 1.5 0 0 1 14.5 5.5V12A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12Z";
const PAGE: &str = "M3.5 2.5A1 1 0 0 1 4.5 1.5H9.5L12.5 4.5V13.5A1 1 0 0 1 11.5 14.5H4.5A1 1 0 0 1 3.5 13.5Z M9.5 1.5V4.5H12.5";
const STAR: &str =
    "M8 1.6L9.9 5.6L14.3 6.1L11 9.1L11.9 13.4L8 11.2L4.1 13.4L5 9.1L1.7 6.1L6.1 5.6Z";
const BIN: &str = "M2 4H14 M6 4V2.5H10V4 M3.5 4L4.4 13.1A1.5 1.5 0 0 0 5.9 14.5H10.1A1.5 1.5 0 0 0 11.6 13.1L12.5 4";
const PIN: &str = "M10 1.8L14.2 6L12.4 6.8L9.9 9.3L10.3 12.3L9.4 13.2L2.8 6.6L3.7 5.7L6.7 6.1L9.2 3.6Z M6.1 9.9L2 14";

fn definition(name: &str) -> Option<&'static [(Paint, &'static str)]> {
    Some(match name {
        "back" => &[(Stroke, "M13.5 8H2.5 M7 3.5L2.5 8L7 12.5")],
        "forward" => &[(Stroke, "M2.5 8H13.5 M9 3.5L13.5 8L9 12.5")],
        "up" => &[(Stroke, "M8 13.5V2.5 M3.5 7L8 2.5L12.5 7")],
        "refresh" => &[(Stroke, "M13.5 8A5.5 5.5 0 1 1 11.9 4.1 M12.2 1.4V4.4H9.2")],
        "chevron-right" => &[(Stroke, "M6 3.5L10.5 8L6 12.5")],
        "chevron-left" => &[(Stroke, "M10 3.5L5.5 8L10 12.5")],
        "chevron-down" => &[(Stroke, "M3.5 6L8 10.5L12.5 6")],
        "chevron-up" => &[(Stroke, "M3.5 10L8 5.5L12.5 10")],
        "search" => &[(
            Stroke,
            "M10.5 6.5A4 4 0 1 1 2.5 6.5A4 4 0 1 1 10.5 6.5Z M9.5 9.5L13.5 13.5",
        )],
        "close" => &[(Stroke, "M3.5 3.5L12.5 12.5 M12.5 3.5L3.5 12.5")],
        "add" => &[(Stroke, "M8 2.5V13.5 M2.5 8H13.5")],
        "more" => &[(
            Fill,
            "M4 8A1 1 0 1 1 2 8A1 1 0 1 1 4 8Z M9 8A1 1 0 1 1 7 8A1 1 0 1 1 9 8Z M14 8A1 1 0 1 1 12 8A1 1 0 1 1 14 8Z",
        )],
        "new" => &[(Stroke, CIRCLE_15), (AccentStroke, "M8 5V11 M5 8H11")],
        "cut" => &[
            (Stroke, "M6.6 10.4L12.2 1.8 M9.4 10.4L3.8 1.8"),
            (
                AccentStroke,
                "M6.8 11.8A2 2 0 1 1 2.8 11.8A2 2 0 1 1 6.8 11.8Z M13.2 11.8A2 2 0 1 1 9.2 11.8A2 2 0 1 1 13.2 11.8Z",
            ),
        ],
        "copy" => &[
            (
                Stroke,
                "M5.5 4.5V3A1.5 1.5 0 0 1 7 1.5H12A1.5 1.5 0 0 1 13.5 3V10A1.5 1.5 0 0 1 12 11.5H10.5",
            ),
            (
                Stroke,
                "M4 4.5H9A1.5 1.5 0 0 1 10.5 6V13A1.5 1.5 0 0 1 9 14.5H4A1.5 1.5 0 0 1 2.5 13V6A1.5 1.5 0 0 1 4 4.5Z",
            ),
        ],
        "paste" => &[
            (
                Stroke,
                "M5.5 2.5H4A1.5 1.5 0 0 0 2.5 4V13A1.5 1.5 0 0 0 4 14.5H12A1.5 1.5 0 0 0 13.5 13V4A1.5 1.5 0 0 0 12 2.5H10.5",
            ),
            (
                AccentStroke,
                "M6.5 1.5H9.5A1 1 0 0 1 10.5 2.5V3A1 1 0 0 1 9.5 4H6.5A1 1 0 0 1 5.5 3V2.5A1 1 0 0 1 6.5 1.5Z",
            ),
        ],
        "rename" => &[
            (
                Stroke,
                "M8.5 4.5H3A1.5 1.5 0 0 0 1.5 6V10A1.5 1.5 0 0 0 3 11.5H8.5 M13.5 4.5A1.5 1.5 0 0 1 14.5 6V10A1.5 1.5 0 0 1 13.5 11.5",
            ),
            (AccentStroke, "M11 2.5V13.5 M9.5 2.5H12.5 M9.5 13.5H12.5"),
        ],
        "share" => &[
            (
                Stroke,
                "M2.5 9.5V12.5A1 1 0 0 0 3.5 13.5H12.5A1 1 0 0 0 13.5 12.5V9.5",
            ),
            (AccentStroke, "M5 10C5 6.8 7.2 5 11 5 M9 2.5L11.5 5L9 7.5"),
        ],
        "delete" => &[(Stroke, BIN), (Stroke, "M6.5 7V11.5 M9.5 7V11.5")],
        "empty-trash" => &[
            (Stroke, BIN),
            (AccentStroke, "M6.2 7.5L9.8 11.5 M9.8 7.5L6.2 11.5"),
        ],
        "sort" => &[
            (Stroke, "M5 2.5V13.5 M2.5 5L5 2.5L7.5 5"),
            (AccentStroke, "M11 13.5V2.5 M8.5 11L11 13.5L13.5 11"),
        ],
        "view" => &[(
            Stroke,
            "M3.5 2.5H6A1 1 0 0 1 7 3.5V6A1 1 0 0 1 6 7H3.5A1 1 0 0 1 2.5 6V3.5A1 1 0 0 1 3.5 2.5Z \
             M10 2.5H12.5A1 1 0 0 1 13.5 3.5V6A1 1 0 0 1 12.5 7H10A1 1 0 0 1 9 6V3.5A1 1 0 0 1 10 2.5Z \
             M3.5 9H6A1 1 0 0 1 7 10V12.5A1 1 0 0 1 6 13.5H3.5A1 1 0 0 1 2.5 12.5V10A1 1 0 0 1 3.5 9Z \
             M10 9H12.5A1 1 0 0 1 13.5 10V12.5A1 1 0 0 1 12.5 13.5H10A1 1 0 0 1 9 12.5V10A1 1 0 0 1 10 9Z",
        )],
        "filter" => &[(Stroke, "M2 3H14L9.5 8.5V12.8L6.5 14.3V8.5Z")],
        "details-pane" => &[
            (
                Stroke,
                "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z M9.5 2.5V13.5",
            ),
            (AccentStroke, "M11 5.5H12 M11 8H12 M11 10.5H12"),
        ],
        "preview-pane" => &[
            (
                Stroke,
                "M1.5 8C3 5 5.3 3.5 8 3.5C10.7 3.5 13 5 14.5 8C13 11 10.7 12.5 8 12.5C5.3 12.5 3 11 1.5 8Z",
            ),
            (AccentStroke, "M10 8A2 2 0 1 1 6 8A2 2 0 1 1 10 8Z"),
        ],
        "navigation-pane" => &[
            (
                Stroke,
                "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z M6.5 2.5V13.5",
            ),
            (AccentStroke, "M4 5.5H5 M4 8H5 M4 10.5H5"),
        ],
        "undo" => &[(
            Stroke,
            "M2.5 6.5H10A3.5 3.5 0 0 1 10 13.5H6 M5.5 3.5L2.5 6.5L5.5 9.5",
        )],
        "redo" => &[(
            Stroke,
            "M13.5 6.5H6A3.5 3.5 0 0 0 6 13.5H10 M10.5 3.5L13.5 6.5L10.5 9.5",
        )],
        "select-all" => &[
            (
                AccentStroke,
                "M2 3.5L3.2 4.7L5.2 2.5 M2 8L3.2 9.2L5.2 7 M2 12.5L3.2 13.7L5.2 11.5",
            ),
            (Stroke, "M7.5 3.5H14 M7.5 8H14 M7.5 12.5H14"),
        ],
        "select-none" => &[(
            Stroke,
            "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z",
        )],
        "invert-selection" => &[
            (
                Stroke,
                "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z",
            ),
            (AccentFill, "M3 3H8V13H3Z"),
        ],
        "properties" => &[
            (Stroke, CIRCLE_15),
            (Stroke, "M8 7V11.5"),
            (Fill, "M8.9 5A.9.9 0 1 1 7.1 5A.9.9 0 1 1 8.9 5Z"),
        ],
        "options" => &[(
            Stroke,
            "M2.5 4.5H4 M7 4.5H13.5 M7 4.5A1.5 1.5 0 1 1 4 4.5A1.5 1.5 0 1 1 7 4.5Z \
             M2.5 11.5H9 M12 11.5H13.5 M12 11.5A1.5 1.5 0 1 1 9 11.5A1.5 1.5 0 1 1 12 11.5Z",
        )],
        "pin" => &[(Stroke, PIN)],
        "unpin" => &[(Stroke, PIN), (AccentStroke, "M1.5 1.5L14.5 14.5")],
        "star" => &[(Stroke, STAR)],
        "star-filled" => &[(ColorFill(0xF2B200), STAR), (ColorStroke(0xE09A00), STAR)],
        "zip" => &[
            (
                Stroke,
                "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z",
            ),
            (
                AccentStroke,
                "M7.2 2.5V4 M8.8 4V5.5 M7.2 5.5V7 M8.8 7V8.5 M7 8.5H9V11H7Z",
            ),
        ],
        "extract" => &[
            (
                Stroke,
                "M2.5 8.5V12.5A1 1 0 0 0 3.5 13.5H12.5A1 1 0 0 0 13.5 12.5V8.5",
            ),
            (AccentStroke, "M8 10V1.8 M5 4.8L8 1.8L11 4.8"),
        ],
        "copy-path" => &[
            (
                Stroke,
                "M7 4.5L8.5 3A2.5 2.5 0 0 1 12 3L13 4A2.5 2.5 0 0 1 13 7.5L11.5 9 M9 11.5L7.5 13A2.5 2.5 0 0 1 4 13L3 12A2.5 2.5 0 0 1 3 8.5L4.5 7",
            ),
            (AccentStroke, "M6.2 9.8L9.8 6.2"),
        ],
        "open" => &[
            (
                Stroke,
                "M13.5 9V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5H7",
            ),
            (AccentStroke, "M9.5 2.5H13.5V6.5 M13.5 2.5L7.5 8.5"),
        ],
        "open-with" => &[
            (
                Stroke,
                "M3.5 2.5H6A1 1 0 0 1 7 3.5V6A1 1 0 0 1 6 7H3.5A1 1 0 0 1 2.5 6V3.5A1 1 0 0 1 3.5 2.5Z \
                 M10 2.5H12.5A1 1 0 0 1 13.5 3.5V6A1 1 0 0 1 12.5 7H10A1 1 0 0 1 9 6V3.5A1 1 0 0 1 10 2.5Z \
                 M3.5 9H6A1 1 0 0 1 7 10V12.5A1 1 0 0 1 6 13.5H3.5A1 1 0 0 1 2.5 12.5V10A1 1 0 0 1 3.5 9Z",
            ),
            (AccentStroke, "M11.25 9V13.5 M9 11.25H13.5"),
        ],
        "terminal" => &[
            (
                Stroke,
                "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z M5 6L7 8L5 10",
            ),
            (AccentStroke, "M8.5 10.5H11"),
        ],
        "tab-new" => &[
            (
                Stroke,
                "M2.5 6V12.5A1 1 0 0 0 3.5 13.5H12.5A1 1 0 0 0 13.5 12.5V6Z M2.5 6V3.5A1 1 0 0 1 3.5 2.5H7A1 1 0 0 1 8 3.5V6",
            ),
            (AccentStroke, "M8 8.2V11.8 M6.2 10H9.8"),
        ],
        "window-new" => &[
            (
                Stroke,
                "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z M2.5 5.5H13.5",
            ),
            (AccentStroke, "M8 7.5V11.5 M6 9.5H10"),
        ],
        "restore" => &[
            (Stroke, "M2.5 8A5.5 5.5 0 1 0 4.1 4.1 M1.8 1.4V4.4H4.8"),
            (AccentStroke, "M8 5.5V8L9.8 9.2"),
        ],
        "check" => &[(Stroke, "M3 8.5L6.5 12L13 4.5")],
        "pause" => &[(Stroke, "M5.5 3.5V12.5 M10.5 3.5V12.5")],
        "radio" => &[(Fill, "M10.5 8A2.5 2.5 0 1 1 5.5 8A2.5 2.5 0 1 1 10.5 8Z")],
        "folder" => &[(Stroke, FOLDER)],
        "folder-new" => &[(Stroke, FOLDER), (AccentStroke, "M8 6.8V11.2 M5.8 9H10.2")],
        "file" => &[(Stroke, PAGE)],
        "file-new" => &[(Stroke, PAGE), (AccentStroke, "M8 7.5V11.5 M6 9.5H10")],
        "shortcut" => &[
            (Stroke, PAGE),
            (
                AccentStroke,
                "M6 12V10.5A2 2 0 0 1 8 8.5H10 M8.8 7L10.3 8.5L8.8 10",
            ),
        ],
        "drive" => &[
            (
                Stroke,
                "M2.5 9.5L4 4A1.5 1.5 0 0 1 5.5 2.5H10.5A1.5 1.5 0 0 1 12 4L13.5 9.5V12A1.5 1.5 0 0 1 12 13.5H4A1.5 1.5 0 0 1 2.5 12Z M2.5 9.5H13.5",
            ),
            (
                AccentFill,
                "M11.8 11.5A.8.8 0 1 1 10.2 11.5A.8.8 0 1 1 11.8 11.5Z",
            ),
        ],
        "eject" => &[(Stroke, "M8 3L13 9H3Z M3 12.5H13")],
        "network" => &[(
            Stroke,
            "M14 8A6 6 0 1 1 2 8A6 6 0 1 1 14 8Z M2 8H14 M8 2C6 4 6 12 8 14 M8 2C10 4 10 12 8 14",
        )],
        "home" => &[(
            Stroke,
            "M2.5 7L8 2.5L13.5 7V13A.5.5 0 0 1 13 13.5H10V9.5H6V13.5H3A.5.5 0 0 1 2.5 13Z",
        )],
        "gallery" => &[
            (
                Stroke,
                "M3.5 3.5H12.5A1 1 0 0 1 13.5 4.5V11.5A1 1 0 0 1 12.5 12.5H3.5A1 1 0 0 1 2.5 11.5V4.5A1 1 0 0 1 3.5 3.5Z",
            ),
            (AccentStroke, "M2.8 11L6 8L8.5 10.5L10 9L13.2 12"),
        ],
        "computer" => &[(
            Stroke,
            "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V9.5A1 1 0 0 1 12.5 10.5H3.5A1 1 0 0 1 2.5 9.5V3.5A1 1 0 0 1 3.5 2.5Z M6 13.5H10 M8 10.5V13.5",
        )],
        "picture" => &[
            (
                Stroke,
                "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z",
            ),
            (AccentStroke, "M2.8 11.2L6 8L8.5 10.5L10 9L13.2 12.2"),
            (
                AccentFill,
                "M11.5 5.5A1 1 0 1 1 9.5 5.5A1 1 0 1 1 11.5 5.5Z",
            ),
        ],
        "rotate-left" => &[(Stroke, "M2.5 8A5.5 5.5 0 1 0 4.1 4.1 M1.8 1.4V4.4H4.8")],
        "rotate-right" => &[(Stroke, "M13.5 8A5.5 5.5 0 1 1 11.9 4.1 M12.2 1.4V4.4H9.2")],
        "clock" => &[(Stroke, CIRCLE_15), (AccentStroke, "M8 4.5V8L10.5 9.5")],
        "send-to" => &[(Stroke, "M2.5 8H11 M8 5L11 8L8 11 M13.5 3V13")],
        "group" => &[(
            Stroke,
            "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V5.5A1 1 0 0 1 12.5 6.5H3.5A1 1 0 0 1 2.5 5.5V3.5A1 1 0 0 1 3.5 2.5Z \
             M3.5 9.5H12.5A1 1 0 0 1 13.5 10.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V10.5A1 1 0 0 1 3.5 9.5Z",
        )],
        "show" => &[
            (
                Stroke,
                "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z",
            ),
            (AccentStroke, "M5 8.2L7 10.2L11 5.8"),
        ],
        "ascending" => &[(Stroke, "M8 13.5V2.5 M3.5 7L8 2.5L12.5 7")],
        "descending" => &[(Stroke, "M8 2.5V13.5 M3.5 9L8 13.5L12.5 9")],
        "view-extra-large" => &[(
            Stroke,
            "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z",
        )],
        "view-large" => &[(
            Stroke,
            "M3.5 2.5H6A1 1 0 0 1 7 3.5V6A1 1 0 0 1 6 7H3.5A1 1 0 0 1 2.5 6V3.5A1 1 0 0 1 3.5 2.5Z \
             M10 2.5H12.5A1 1 0 0 1 13.5 3.5V6A1 1 0 0 1 12.5 7H10A1 1 0 0 1 9 6V3.5A1 1 0 0 1 10 2.5Z \
             M3.5 9H6A1 1 0 0 1 7 10V12.5A1 1 0 0 1 6 13.5H3.5A1 1 0 0 1 2.5 12.5V10A1 1 0 0 1 3.5 9Z \
             M10 9H12.5A1 1 0 0 1 13.5 10V12.5A1 1 0 0 1 12.5 13.5H10A1 1 0 0 1 9 12.5V10A1 1 0 0 1 10 9Z",
        )],
        "view-medium" => &[(
            Stroke,
            "M2.5 2.5H5V5H2.5Z M6.75 2.5H9.25V5H6.75Z M11 2.5H13.5V5H11Z \
             M2.5 6.75H5V9.25H2.5Z M6.75 6.75H9.25V9.25H6.75Z M11 6.75H13.5V9.25H11Z \
             M2.5 11H5V13.5H2.5Z M6.75 11H9.25V13.5H6.75Z M11 11H13.5V13.5H11Z",
        )],
        "view-small" => &[
            (
                Stroke,
                "M2.5 3H4.5V5H2.5Z M9 3H11V5H9Z M2.5 7H4.5V9H2.5Z M9 7H11V9H9Z M2.5 11H4.5V13H2.5Z M9 11H11V13H9Z",
            ),
            (
                Stroke,
                "M6 4H7.5 M12.5 4H14 M6 8H7.5 M12.5 8H14 M6 12H7.5 M12.5 12H14",
            ),
        ],
        "view-list" => &[(
            Stroke,
            "M2.5 4H3.5 M6 4H13.5 M2.5 8H3.5 M6 8H13.5 M2.5 12H3.5 M6 12H13.5",
        )],
        "view-details" => &[(
            Stroke,
            "M3.5 2.5H12.5A1 1 0 0 1 13.5 3.5V12.5A1 1 0 0 1 12.5 13.5H3.5A1 1 0 0 1 2.5 12.5V3.5A1 1 0 0 1 3.5 2.5Z \
             M2.5 6.2H13.5 M2.5 9.8H13.5 M6.5 2.5V13.5",
        )],
        "view-tiles" => &[(
            Stroke,
            "M2.5 2.5H6.5V6.5H2.5Z M8.5 3.5H13.5 M8.5 5.5H11.5 M2.5 9.5H6.5V13.5H2.5Z M8.5 10.5H13.5 M8.5 12.5H11.5",
        )],
        "view-content" => &[(
            Stroke,
            "M2.5 2.5H5.5V5.5H2.5Z M7.5 3H13.5 M7.5 5H10.5 M2.5 7.5H13.5 M2.5 9.5H5.5V12.5H2.5Z M7.5 10H13.5 M7.5 12H10.5",
        )],
        "compact" => &[(
            Stroke,
            "M8 1.5V6 M5.5 3.5L8 6L10.5 3.5 M8 14.5V10 M5.5 12.5L8 10L10.5 12.5 M2.5 8H13.5",
        )],
        "checkboxes" => &[
            (
                Stroke,
                "M3 2.5H6A.5.5 0 0 1 6.5 3V6A.5.5 0 0 1 6 6.5H3A.5.5 0 0 1 2.5 6V3A.5.5 0 0 1 3 2.5Z M3 9.5H6A.5.5 0 0 1 6.5 10V13A.5.5 0 0 1 6 13.5H3A.5.5 0 0 1 2.5 13V10A.5.5 0 0 1 3 9.5Z",
            ),
            (AccentStroke, "M3.4 4.6L4.3 5.5L5.8 3.6"),
            (Stroke, "M8.5 4.5H13.5 M8.5 11.5H13.5"),
        ],
        "extensions" => &[(Stroke, PAGE), (AccentStroke, "M5.5 11.5H10.5")],
        "hidden" => &[
            (
                Stroke,
                "M1.5 8C3 5 5.3 3.5 8 3.5C10.7 3.5 13 5 14.5 8C13 11 10.7 12.5 8 12.5C5.3 12.5 3 11 1.5 8Z M10 8A2 2 0 1 1 6 8A2 2 0 1 1 10 8Z",
            ),
            (AccentStroke, "M2.5 13.5L13.5 2.5"),
        ],
        "minimize" => &[(Stroke, "M3 8H13")],
        "maximize" => &[(Stroke, "M3.5 3.5H12.5V12.5H3.5Z")],
        "unmaximize" => &[(
            Stroke,
            "M3.5 5.5H10.5V12.5H3.5Z M5.5 5.5V3.5H12.5V10.5H10.5",
        )],
        "warning" => &[
            (
                ColorFill(0xF2A900),
                "M8 1.8L14.7 13.6A.6.6 0 0 1 14.2 14.5H1.8A.6.6 0 0 1 1.3 13.6Z",
            ),
            (ColorStroke(0x1A1A1A), "M8 6V9.6"),
            (
                ColorFill(0x1A1A1A),
                "M8.8 11.8A.8.8 0 1 1 7.2 11.8A.8.8 0 1 1 8.8 11.8Z",
            ),
        ],
        "error" => &[
            (
                ColorFill(0xD13438),
                "M14.5 8A6.5 6.5 0 1 1 1.5 8A6.5 6.5 0 1 1 14.5 8Z",
            ),
            (
                ColorStroke(0xFFFFFF),
                "M5.8 5.8L10.2 10.2 M10.2 5.8L5.8 10.2",
            ),
        ],
        "question" => &[
            (
                AccentFill,
                "M14.5 8A6.5 6.5 0 1 1 1.5 8A6.5 6.5 0 1 1 14.5 8Z",
            ),
            (
                ColorStroke(0xFFFFFF),
                "M6.2 6.3A1.9 1.9 0 1 1 8.6 8.1C8.2 8.3 8 8.6 8 9V9.5",
            ),
            (
                ColorFill(0xFFFFFF),
                "M8.7 11.6A.7.7 0 1 1 7.3 11.6A.7.7 0 1 1 8.7 11.6Z",
            ),
        ],
        "info" => &[
            (
                AccentFill,
                "M14.5 8A6.5 6.5 0 1 1 1.5 8A6.5 6.5 0 1 1 14.5 8Z",
            ),
            (ColorStroke(0xFFFFFF), "M8 7.2V11.4"),
            (
                ColorFill(0xFFFFFF),
                "M8.7 4.9A.7.7 0 1 1 7.3 4.9A.7.7 0 1 1 8.7 4.9Z",
            ),
        ],
        "link-overlay" => &[
            (ColorFill(0xFFFFFF), "M1 1H15V15H1Z"),
            (
                ColorStroke(0x2372DD),
                "M1 1H15V15H1Z M5 11.5V9.5A3 3 0 0 1 8 6.5H11 M9 4.5L11 6.5L9 8.5",
            ),
        ],
        _ => return None,
    })
}

/// A glyph's parts, parsed once.
struct Parsed(Vec<(Paint, gsk::Path)>);

thread_local! {
    static PARSED: RefCell<HashMap<String, Option<Rc<Parsed>>>> = RefCell::default();
}

fn parsed(name: &str) -> Option<Rc<Parsed>> {
    PARSED.with(|cache| {
        cache
            .borrow_mut()
            .entry(name.to_owned())
            .or_insert_with(|| {
                let parts = definition(name)?
                    .iter()
                    .map(|(paint, data)| {
                        let path = gsk::Path::parse(data)
                            .unwrap_or_else(|_| panic!("glyph {name} has a bad path: {data}"));
                        (*paint, path)
                    })
                    .collect();
                Some(Rc::new(Parsed(parts)))
            })
            .clone()
    })
}

fn rgb(hex: u32) -> gdk::RGBA {
    gdk::RGBA::new(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        1.0,
    )
}

/// Patrone Blue, lighter on dark backgrounds so it keeps its contrast.
pub fn accent_for(foreground: &gdk::RGBA) -> gdk::RGBA {
    let light_text =
        foreground.red() * 0.3 + foreground.green() * 0.59 + foreground.blue() * 0.11 > 0.5;
    let mut accent = rgb(if light_text { 0x4DA0FC } else { 0x1F7AE8 });
    accent.set_alpha(foreground.alpha().max(0.45));
    accent
}

mod imp {
    use super::*;

    pub struct Glyph {
        pub name: RefCell<String>,
        pub size: Cell<i32>,
    }

    impl Default for Glyph {
        fn default() -> Self {
            Self {
                name: RefCell::default(),
                size: Cell::new(16),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Glyph {
        const NAME: &'static str = "PfmGlyph";
        type Type = super::Glyph;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("glyph");
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for Glyph {}

    impl WidgetImpl for Glyph {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let size = self.size.get();
            (size, size, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let Some(parsed) = parsed(&self.name.borrow()) else {
                return;
            };
            let size = self.size.get() as f32;
            let color = widget.color();
            let accent = accent_for(&color);
            let x = ((widget.width() as f32 - size) / 2.0).round();
            let y = ((widget.height() as f32 - size) / 2.0).round();
            snapshot.save();
            snapshot.translate(&graphene::Point::new(x, y));
            snapshot.scale(size / 16.0, size / 16.0);
            let stroke = gsk::Stroke::new(1.0);
            stroke.set_line_cap(gsk::LineCap::Round);
            stroke.set_line_join(gsk::LineJoin::Round);
            for (paint, path) in &parsed.0 {
                match *paint {
                    Stroke => snapshot.append_stroke(path, &stroke, &color),
                    AccentStroke => snapshot.append_stroke(path, &stroke, &accent),
                    Fill => snapshot.append_fill(path, gsk::FillRule::Winding, &color),
                    AccentFill => snapshot.append_fill(path, gsk::FillRule::Winding, &accent),
                    ColorFill(hex) => snapshot.append_fill(path, gsk::FillRule::Winding, &rgb(hex)),
                    ColorStroke(hex) => snapshot.append_stroke(path, &stroke, &rgb(hex)),
                }
            }
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    pub struct Glyph(ObjectSubclass<imp::Glyph>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Glyph {
    pub fn new(name: &str) -> Self {
        Self::sized(name, 16)
    }

    pub fn sized(name: &str, size: i32) -> Self {
        let glyph: Self = glib::Object::new();
        glyph.imp().size.set(size);
        glyph.set_name(name);
        glyph
    }

    pub fn set_name(&self, name: &str) {
        debug_assert!(definition(name).is_some(), "unknown glyph {name}");
        if *self.imp().name.borrow() != name {
            self.imp().name.replace(name.to_owned());
            self.queue_draw();
        }
    }

    pub fn set_size(&self, size: i32) {
        self.imp().size.set(size);
        self.queue_resize();
    }
}

/// A flat button showing a glyph, with a tooltip.
pub fn button(name: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .child(&Glyph::new(name))
        .tooltip_text(tooltip)
        .build();
    button.add_css_class("flat");
    button.add_css_class("glyph-button");
    button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[&str] = &[
        "back",
        "forward",
        "up",
        "refresh",
        "chevron-right",
        "chevron-left",
        "chevron-down",
        "chevron-up",
        "search",
        "close",
        "add",
        "more",
        "new",
        "cut",
        "copy",
        "paste",
        "rename",
        "share",
        "delete",
        "empty-trash",
        "sort",
        "view",
        "filter",
        "details-pane",
        "preview-pane",
        "navigation-pane",
        "undo",
        "redo",
        "select-all",
        "select-none",
        "invert-selection",
        "properties",
        "options",
        "pin",
        "unpin",
        "star",
        "star-filled",
        "zip",
        "extract",
        "copy-path",
        "open",
        "open-with",
        "terminal",
        "tab-new",
        "window-new",
        "restore",
        "check",
        "pause",
        "radio",
        "folder",
        "folder-new",
        "file",
        "file-new",
        "shortcut",
        "drive",
        "eject",
        "network",
        "home",
        "gallery",
        "computer",
        "picture",
        "rotate-left",
        "rotate-right",
        "clock",
        "send-to",
        "group",
        "show",
        "ascending",
        "descending",
        "view-extra-large",
        "view-large",
        "view-medium",
        "view-small",
        "view-list",
        "view-details",
        "view-tiles",
        "view-content",
        "compact",
        "checkboxes",
        "extensions",
        "hidden",
        "minimize",
        "maximize",
        "unmaximize",
        "warning",
        "error",
        "question",
        "info",
        "link-overlay",
    ];

    #[test]
    fn every_glyph_parses() {
        for name in ALL {
            for (_, data) in definition(name).unwrap() {
                assert!(gsk::Path::parse(data).is_ok(), "{name}: {data}");
            }
        }
    }
}
