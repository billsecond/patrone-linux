// The taskbar's color taken from the wallpaper: the picture's average hue,
// at the theme's own lightness, so the bar sits in the picture (Patrone's
// navy wallpaper gives a navy bar, Graphite a graphite one) instead of a
// grey that matches nothing. The user asked for it on 2026-10-26.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GdkPixbuf from 'gi://GdkPixbuf';

const BACKGROUND_SCHEMA = 'org.gnome.desktop.background';
const INTERFACE_SCHEMA = 'org.gnome.desktop.interface';

// The picture is decoded in a thread, at this size, which is plenty for an
// average
const SAMPLE_WIDTH = 64;
const SAMPLE_HEIGHT = 36;

// The themes that take the wallpaper's color: the bar's lightness and
// opacity in each style (near the themes' own greys, so their text and
// icons read the same), and its line's, where it has one. Classic and
// Royal keep their own colors.
const LOOKS = {
    patrone: {
        dark: {l: 0.12, a: 0.94, line: {l: 0.6, a: 0.28}},
        light: {l: 0.94, a: 0.94, line: {l: 0.45, a: 0.22}},
    },
    modern: {
        dark: {l: 0.12, a: 0.94},
        light: {l: 0.95, a: 0.94},
    },
    studio: {
        dark: {l: 0.14, a: 0.74},
        light: {l: 0.95, a: 0.72},
    },
};
// A vivid wallpaper still gives a calm bar
const MAX_SATURATION = {dark: 0.5, light: 0.45};

function rgbToHsl(r, g, b) {
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    const l = (max + min) / 2;
    if (max === min)
        return {h: 0, s: 0, l};
    const d = max - min;
    const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    let h;
    if (max === r)
        h = (g - b) / d + (g < b ? 6 : 0);
    else if (max === g)
        h = (b - r) / d + 2;
    else
        h = (r - g) / d + 4;
    return {h: h / 6, s, l};
}

function hslToRgb(h, s, l) {
    const channel = t => {
        const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
        const p = 2 * l - q;
        t = (t + 1) % 1;
        if (t < 1 / 6)
            return p + (q - p) * 6 * t;
        if (t < 1 / 2)
            return q;
        if (t < 2 / 3)
            return p + (q - p) * (2 / 3 - t) * 6;
        return p;
    };
    return [channel(h + 1 / 3), channel(h), channel(h - 1 / 3)].map(c => Math.round(c * 255));
}

function rgba(hue, saturation, {l, a}) {
    return `rgba(${hslToRgb(hue.h, saturation, l).join(', ')}, ${a})`;
}

/** The average color of a picture, as hue, saturation and lightness. */
function averageColor(pixbuf) {
    const pixels = pixbuf.get_pixels();
    const channels = pixbuf.get_n_channels();
    const stride = pixbuf.get_rowstride();
    let r = 0, g = 0, b = 0;
    for (let y = 0; y < pixbuf.height; y++) {
        for (let x = 0; x < pixbuf.width; x++) {
            const i = y * stride + x * channels;
            r += pixels[i];
            g += pixels[i + 1];
            b += pixels[i + 2];
        }
    }
    const count = pixbuf.width * pixbuf.height * 255;
    return rgbToHsl(r / count, g / count, b / count);
}

function parseColor(text) {
    const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(text ?? '');
    if (!match)
        return null;
    const [r, g, b] = match.slice(1).map(hex => parseInt(hex, 16) / 255);
    return rgbToHsl(r, g, b);
}

export class WallpaperColor {
    /**
     * @param {Function} onChanged - called once the wallpaper's color is
     *   known, and whenever it changes
     */
    constructor(onChanged) {
        this._onChanged = onChanged;
        this._cancellable = null;
        /** The wallpaper's average color, or null until it's known */
        this.color = null;

        this._background = new Gio.Settings({schema_id: BACKGROUND_SCHEMA});
        this._interface = new Gio.Settings({schema_id: INTERFACE_SCHEMA});
        this._background.connectObject('changed', () => this._load(), this);
        // The desktop shows picture-uri-dark in the dark style
        this._interface.connectObject('changed::color-scheme', () => this._load(), this);
        this._load();
    }

    destroy() {
        this._cancellable?.cancel();
        this._background.disconnectObject(this);
        this._interface.disconnectObject(this);
    }

    /**
     * The bar's style for a theme in a style ("dark" or "light"), or null
     * where the theme keeps its own colors or the color isn't known yet.
     *
     * @param {string} theme
     * @param {string} variant
     * @returns {string|null}
     */
    styleFor(theme, variant) {
        const look = LOOKS[theme]?.[variant];
        if (!look || !this.color)
            return null;
        const saturation = Math.min(this.color.s, MAX_SATURATION[variant]);
        let style = `background-color: ${rgba(this.color, saturation, look)};`;
        if (look.line)
            style += ` border-color: ${rgba(this.color, Math.min(this.color.s, 0.8), look.line)};`;
        return style;
    }

    _set(color) {
        this.color = color;
        this._onChanged();
    }

    _load() {
        this._cancellable?.cancel();
        const cancellable = this._cancellable = new Gio.Cancellable();

        const dark = this._interface.get_string('color-scheme') === 'prefer-dark';
        const uri = (dark && this._background.get_string('picture-uri-dark')) ||
            this._background.get_string('picture-uri');
        if (!uri || this._background.get_string('picture-options') === 'none') {
            this._set(parseColor(this._background.get_string('primary-color')));
            return;
        }

        // A slideshow's XML, or a missing picture, gives no color: the
        // theme's own then
        const failed = error => {
            if (!(error instanceof GLib.Error && error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED)))
                this._set(null);
        };
        Gio.File.new_for_uri(uri).read_async(GLib.PRIORITY_LOW, cancellable, (file, result) => {
            let stream;
            try {
                stream = file.read_finish(result);
            } catch (error) {
                failed(error);
                return;
            }
            GdkPixbuf.Pixbuf.new_from_stream_at_scale_async(stream,
                SAMPLE_WIDTH, SAMPLE_HEIGHT, false, cancellable, (_, loaded) => {
                    stream.close_async(GLib.PRIORITY_LOW, null, null);
                    try {
                        this._set(averageColor(GdkPixbuf.Pixbuf.new_from_stream_finish(loaded)));
                    } catch (error) {
                        failed(error);
                    }
                });
        });
    }
}
