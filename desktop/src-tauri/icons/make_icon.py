"""Generate the Embeder app icon set from a single vector-style master rendered with
Pillow. The mark: a phosphor-green oscilloscope trace forming a checkmark on a dark
instrument tile — "signal, verified." Re-run to regenerate all sizes + the favicon.

    python desktop/src-tauri/icons/make_icon.py
"""
import os
from PIL import Image, ImageDraw, ImageFilter

HERE = os.path.dirname(os.path.abspath(__file__))
DIST = os.path.normpath(os.path.join(HERE, "..", "..", "dist"))

BG_TOP = (18, 24, 31, 255)      # #12181f
BG_BOT = (11, 15, 20, 255)      # #0b0f14
BORDER = (42, 49, 60, 255)      # #2a313c
GRID = (40, 90, 70, 120)        # dim green baseline
TRACE = (110, 231, 168, 255)    # #6ee7a8 bright phosphor
GLOW = (76, 195, 138, 180)      # #4cc38a


def _rounded_mask(size, radius):
    m = Image.new("L", (size, size), 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=255)
    return m


def _thick_polyline(draw, pts, width, color):
    draw.line(pts, fill=color, width=width, joint="curve")
    r = width // 2
    for (x, y) in pts:  # round the caps + joints
        draw.ellipse([x - r, y - r, x + r, y + r], fill=color)


def render(px=1024):
    # Vertical gradient background.
    grad = Image.new("RGBA", (px, px))
    gpx = grad.load()
    for y in range(px):
        t = y / (px - 1)
        gpx_row = tuple(int(BG_TOP[i] + (BG_BOT[i] - BG_TOP[i]) * t) for i in range(4))
        for x in range(px):
            gpx[x, y] = gpx_row
    mask = _rounded_mask(px, radius=int(px * 0.22))
    tile = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    tile.paste(grad, (0, 0), mask)

    # Inner border.
    bd = ImageDraw.Draw(tile)
    inset = int(px * 0.02)
    bd.rounded_rectangle(
        [inset, inset, px - 1 - inset, px - 1 - inset],
        radius=int(px * 0.20), outline=BORDER, width=max(2, px // 200),
    )

    s = px / 1024.0
    check = [(250 * s, 560 * s), (430 * s, 730 * s), (790 * s, 330 * s)]

    # Dashed scope baseline behind the trace.
    by = int(802 * s)
    x = int(190 * s)
    while x < int(834 * s):
        bd.line([(x, by), (x + int(46 * s), by)], fill=GRID, width=max(2, int(9 * s)))
        x += int(78 * s)

    # Green glow, then the sharp trace on top.
    glow = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    _thick_polyline(ImageDraw.Draw(glow), check, int(112 * s), GLOW)
    glow = glow.filter(ImageFilter.GaussianBlur(int(26 * s)))
    tile = Image.alpha_composite(tile, Image.composite(glow, Image.new("RGBA", (px, px), (0, 0, 0, 0)), mask))

    _thick_polyline(ImageDraw.Draw(tile), check, int(72 * s), TRACE)
    # Keep everything inside the rounded tile.
    out = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    out.paste(tile, (0, 0), mask)
    return out


def main():
    master = render(1024)
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
        "Square150x150Logo.png": 150,
        "StoreLogo.png": 50,
    }
    for name, size in sizes.items():
        master.resize((size, size), Image.LANCZOS).save(os.path.join(HERE, name))

    ico_sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    master.resize((256, 256), Image.LANCZOS).save(os.path.join(HERE, "icon.ico"), sizes=ico_sizes)

    # Web app identity: favicon for the dev-server UI.
    if os.path.isdir(DIST):
        master.resize((256, 256), Image.LANCZOS).save(os.path.join(DIST, "favicon.ico"), sizes=[(16, 16), (32, 32), (48, 48)])
        master.resize((180, 180), Image.LANCZOS).save(os.path.join(DIST, "favicon.png"))

    print("icons written to", HERE)
    if os.path.isdir(DIST):
        print("favicon written to", DIST)


if __name__ == "__main__":
    main()
