"""Render the README animations.

Frames come from the project's own renderer (`slash-claude frames ...`), so the GIFs show
exactly what the code does. They are drawn onto an illustration of the laptop lid: a diagonal
strip of 63 small lights, grouped into 7 segments of 9. The hardware can only set a whole
segment at a time, so all 9 lights of a segment always share one brightness.

    cargo build --release
    pip install pillow
    python docs/generate_gifs.py            # writes docs/*.gif

Set SLASH_CLAUDE_EXE to use a different binary.
"""
import math
import os
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
EXE = os.environ.get("SLASH_CLAUDE_EXE") or str(ROOT / "target" / "release" / "slash-claude.exe")
OUT = ROOT / "docs"

W, H = 760, 640
SS = 2            # supersampling for crisp edges
FPS = 14
GAMMA = 1.9       # display gamma applied to brightness values
N = 7             # controllable segments
PER = 9           # small lights per segment (not individually controllable)

# --- colours -------------------------------------------------------------
BG = (13, 17, 23)
LID = (9, 9, 11)
LID_HATCH = (19, 19, 23)
LID_EDGE = (58, 60, 67)
STRIP_EDGE = (92, 96, 106)
DASH_OFF = (46, 49, 56)
LIGHT = (236, 243, 255)
TITLE_C, SUB_C = (240, 244, 250), (150, 160, 175)

# --- lid + strip geometry (1x pixels) --------------------------------------
LID_BOX = (20, 20, 740, 520)
# The strip runs corner to corner; its end caps are horizontal cuts.
TOP_L, TOP_R, TOP_Y = 100.0, 125.0, 89.0
BOT_L, BOT_R, BOT_Y = 623.5, 647.5, 453.0
DASH_LEN, DASH_W, DASH_TILT = 15.0, 3.0, 1.4
BAR_BOX = (60, 50, 700, 500)     # region used to build the GIF palette


def font(bold, size):
    name = "segoeuib.ttf" if bold else "segoeui.ttf"
    for path in (os.path.join(r"C:\Windows\Fonts", name), "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"):
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            continue
    return ImageFont.load_default()


def dash_centers():
    """63 dash centres along the strip's centre line, top-left to bottom-right."""
    x0, x1 = (TOP_L + TOP_R) / 2, (BOT_L + BOT_R) / 2
    total = N * PER
    return [(x0 + (x1 - x0) * (i + 0.5) / total, TOP_Y + (BOT_Y - TOP_Y) * (i + 0.5) / total)
            for i in range(total)]


def dash_line(cx, cy, scale=1):
    h = DASH_LEN / 2
    return [((cx - h) * scale, (cy + DASH_TILT) * scale), ((cx + h) * scale, (cy - DASH_TILT) * scale)]


def static_background():
    """Laptop lid seen from behind with the unlit strip. Drawn at 2x, returned at 1x."""
    s = SS
    img = Image.new("RGB", (W * s, H * s), BG)

    # soft shadow around the lid
    shadow = Image.new("L", img.size, 0)
    x0, y0, x1, y1 = LID_BOX
    ImageDraw.Draw(shadow).rounded_rectangle(((x0 - 6) * s, (y0 + 4) * s, (x1 + 6) * s, (y1 + 14) * s),
                                             radius=34 * s, fill=140)
    shadow = shadow.filter(ImageFilter.GaussianBlur(16 * s))
    img.paste(Image.new("RGB", img.size, (0, 0, 0)), mask=shadow)

    # lid with a fine diagonal hatch texture
    lid_w, lid_h = (x1 - x0) * s, (y1 - y0) * s
    lid = Image.new("RGB", (lid_w, lid_h), LID)
    ld = ImageDraw.Draw(lid)
    step = 5 * s
    for k in range(-lid_h, lid_w, step):
        ld.line((k, lid_h, k + lid_h, 0), fill=LID_HATCH, width=1)
    mask = Image.new("L", lid.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, lid_w - 1, lid_h - 1), radius=26 * s, fill=255)
    img.paste(lid, (x0 * s, y0 * s), mask)
    d = ImageDraw.Draw(img)
    d.rounded_rectangle((x0 * s, y0 * s, x1 * s, y1 * s), radius=26 * s, outline=LID_EDGE, width=s)

    # strip outline and unlit dashes
    outline = [(TOP_L, TOP_Y), (TOP_R, TOP_Y), (BOT_R, BOT_Y), (BOT_L, BOT_Y)]
    d.polygon([(x * s, y * s) for x, y in outline], outline=STRIP_EDGE, width=s)
    for cx, cy in dash_centers():
        d.line(dash_line(cx, cy, s), fill=DASH_OFF, width=int(DASH_W * s))
    return img.resize((W, H), Image.LANCZOS)


def with_caption(bg, title, sub):
    img = bg.copy()
    d = ImageDraw.Draw(img)
    d.text((W // 2, 568), title, font=font(True, 27), fill=TITLE_C, anchor="mm")
    d.text((W // 2, 604), sub, font=font(False, 18), fill=SUB_C, anchor="mm")
    return img


def add_light(base, values):
    """Additively composite lit dashes (9 per segment share one value) plus bloom onto `base`."""
    big = Image.new("RGB", (W * SS, H * SS), (0, 0, 0))
    d = ImageDraw.Draw(big)
    for i, (cx, cy) in enumerate(dash_centers()):
        v = values[i // PER]
        if v > 0.01:
            v = v ** GAMMA  # perceived brightness: keeps 35% clearly dimmer than 100%
            d.line(dash_line(cx, cy, SS), fill=tuple(int(c * v) for c in LIGHT), width=int(DASH_W * SS))
    light = big.resize((W, H), Image.LANCZOS)

    def boost(im, k):
        return im.point([min(255, int(i * k)) for i in range(256)] * 3)

    out = ImageChops.add(base, light)
    out = ImageChops.add(out, boost(light.filter(ImageFilter.GaussianBlur(3)), 0.7))
    out = ImageChops.add(out, boost(light.filter(ImageFilter.GaussianBlur(9)), 1.0))
    out = ImageChops.add(out, boost(light.filter(ImageFilter.GaussianBlur(24)), 1.2))
    return out


def get_frames(state, left, burn, secs, t0):
    run = subprocess.run([EXE, "frames", state, str(left), str(burn), str(secs), str(FPS), str(t0)],
                         capture_output=True, text=True, check=True)
    return [[float(x) for x in line.split()] for line in run.stdout.splitlines() if line.strip()]


def build(name, scenes):
    """scenes: (state, left_pct, burn, seconds, title, subtitle)"""
    bg = static_background()
    frames, t0 = [], 1000.0
    for state, left, burn, secs, title, sub in scenes:
        base = with_caption(bg, title, sub)
        for values in get_frames(state, left, burn, secs, t0):
            frames.append(add_light(base, values))
        t0 += secs

    # One shared palette keeps colours from flickering between frames. Build it mostly from the
    # strip region, plus a dedicated brightness ramp so lit dashes never posterise.
    full = frames[:: max(1, len(frames) // 6)][:6]
    crops = [f.crop(BAR_BOX) for f in frames[:: max(1, len(frames) // 40)]]
    cw, ch = BAR_BOX[2] - BAR_BOX[0], BAR_BOX[3] - BAR_BOX[1]
    sheet = Image.new("RGB", (W, H * len(full) + ch * len(crops)))
    for k, f in enumerate(full):
        sheet.paste(f, (0, k * H))
    for k, c in enumerate(crops):
        sheet.paste(c, (0, H * len(full) + k * ch))
    med = sheet.quantize(colors=160, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    pal = list(med.getpalette()[: 160 * 3])
    for k in range(96):
        t = k / 95
        pal += [min(255, int(DASH_OFF[j] + LIGHT[j] * t)) for j in range(3)]
    pal += [0] * (768 - len(pal))
    palette = Image.new("P", (1, 1))
    palette.putpalette(pal)
    q = [f.quantize(palette=palette, dither=Image.Dither.NONE) for f in frames]

    path = OUT / f"{name}.gif"
    q[0].save(path, save_all=True, append_images=q[1:], duration=round(1000 / FPS), loop=0, optimize=True, disposal=1)
    print(f"{path.name}: {len(frames)} frames, {path.stat().st_size / 1024:.0f} KiB")


def main():
    if not os.path.exists(EXE):
        sys.exit(f"binary not found: {EXE} (run `cargo build --release`)")
    OUT.mkdir(exist_ok=True)

    idle = lambda left, secs, t="Idle: credits left", s=None: (
        "idle", left, 0.0, secs, t, s or f"remaining 5-hour credits: {left:.0f}%")

    # hero: the whole story in one loop
    build("overview", [
        idle(94, 3.0, "Idle", "bar shows your remaining 5-hour credits (94%)"),
        ("working", 94, 0.3, 4.5, "Prompt running", "credits bar dims, a bright segment runs over it"),
        ("working", 94, 2.0, 3.0, "Burning through credits fast", "the runner speeds up with your usage rate"),
        ("waiting", 94, 0.0, 3.5, "Claude needs your input", "permission prompt or question: the whole bar blinks"),
        idle(93, 2.5, "Done", "back to your remaining credits"),
    ])

    build("credits", [idle(v, 1.6) for v in (100, 70, 40, 20)] +
          [idle(6, 3.0, "Almost out", "under 10% the first segment blinks")])

    build("running", [
        ("working", 70, 0.0, 3.0, "Prompt running", "normal pace"),
        ("working", 70, 2.0, 3.0, "Prompt running", "high burn rate: faster runner"),
    ])

    build("waiting", [("waiting", 70, 0.0, 3.5, "Claude needs your input",
                       "permission prompt, option question or plan approval")])


if __name__ == "__main__":
    main()
