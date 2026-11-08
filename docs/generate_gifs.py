"""Render the README animations.

Frames come from the project's own renderer (`slash-claude frames ...`), so the GIFs show
exactly what the code does. They are drawn onto a stylised illustration of a laptop lid
(not a photo of real hardware).

    cargo build --release
    pip install pillow
    python docs/generate_gifs.py            # writes docs/*.gif

Set SLASH_CLAUDE_EXE to use a different binary.
"""
import os
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
EXE = os.environ.get("SLASH_CLAUDE_EXE") or str(ROOT / "target" / "release" / "slash-claude.exe")
OUT = ROOT / "docs"

W, H = 720, 440
SS = 2            # supersampling for crisp edges
FPS = 14
GAMMA = 1.9       # display gamma applied to brightness values
N = 7             # segments

# --- colours -------------------------------------------------------------
BG = (13, 17, 23)
LID_TOP, LID_BOT = (52, 55, 63), (31, 33, 38)
LID_EDGE = (78, 82, 92)
TRACK = (16, 17, 20)
SEG_OFF = (38, 41, 47)
LIGHT = (236, 243, 255)
TITLE_C, SUB_C = (240, 244, 250), (150, 160, 175)

# --- slash bar geometry (1x pixels) ---------------------------------------
SEG_W, SEG_GAP, SEG_H, SKEW = 34, 15, 30, 12
BAR_W = N * SEG_W + (N - 1) * SEG_GAP
BAR_X = (W - BAR_W) // 2
BAR_Y = 232
BAR_BOX = (110, 170, 610, 300)   # region around the bar (bar + bloom)


def font(bold, size):
    name = "segoeuib.ttf" if bold else "segoeui.ttf"
    for base in (r"C:\Windows\Fonts", "/usr/share/fonts/truetype/dejavu"):
        try:
            return ImageFont.truetype(os.path.join(base, name if base.startswith("C") else "DejaVuSans.ttf"), size)
        except OSError:
            continue
    return ImageFont.load_default()


def seg_poly(i, scale=1):
    """Slanted '/' parallelogram for segment i, left to right."""
    x0 = BAR_X + i * (SEG_W + SEG_GAP)
    pts = [(x0 + SKEW, BAR_Y), (x0 + SKEW + SEG_W, BAR_Y), (x0 + SEG_W, BAR_Y + SEG_H), (x0, BAR_Y + SEG_H)]
    return [(x * scale, y * scale) for x, y in pts]


def vertical_gradient(size, top, bottom):
    w, h = size
    mask = Image.linear_gradient("L").resize((w, h))
    return Image.composite(Image.new("RGB", size, bottom), Image.new("RGB", size, top), mask)


def static_background():
    """Laptop lid seen from behind, with the (unlit) bar track. Drawn at 2x, returned at 1x."""
    s = SS
    img = Image.new("RGB", (W * s, H * s), BG)
    d = ImageDraw.Draw(img)

    # soft floor shadow under the laptop
    shadow = Image.new("L", (W * s, H * s), 0)
    ImageDraw.Draw(shadow).ellipse((90 * s, 318 * s, 630 * s, 352 * s), fill=120)
    shadow = shadow.filter(ImageFilter.GaussianBlur(14 * s))
    img.paste(Image.new("RGB", img.size, (0, 0, 0)), mask=shadow)

    # hinge + base slab
    d.rounded_rectangle((70 * s, 312 * s, 650 * s, 334 * s), radius=10 * s, fill=(26, 28, 32), outline=(40, 43, 48), width=s)
    d.rounded_rectangle((140 * s, 296 * s, 580 * s, 318 * s), radius=8 * s, fill=(22, 23, 27))

    # lid with vertical gradient
    lx0, ly0, lx1, ly1 = 90 * s, 36 * s, 630 * s, 300 * s
    lid = vertical_gradient((lx1 - lx0, ly1 - ly0), LID_TOP, LID_BOT)
    mask = Image.new("L", lid.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, lid.width - 1, lid.height - 1), radius=24 * s, fill=255)
    img.paste(lid, (lx0, ly0), mask)
    d.rounded_rectangle((lx0, ly0, lx1, ly1), radius=24 * s, outline=LID_EDGE, width=s)
    # faint top highlight
    d.line((lx0 + 30 * s, ly0 + 2 * s, lx1 - 30 * s, ly0 + 2 * s), fill=(96, 100, 110), width=s)

    # recessed track + unlit segments
    tx0, ty0 = (BAR_X - 22) * s, (BAR_Y - 18) * s
    tx1, ty1 = (BAR_X + BAR_W + SKEW + 22) * s, (BAR_Y + SEG_H + 18) * s
    d.rounded_rectangle((tx0, ty0, tx1, ty1), radius=18 * s, fill=TRACK, outline=(10, 10, 12), width=2 * s)
    for i in range(N):
        d.polygon(seg_poly(i, s), fill=SEG_OFF)
    return img.resize((W, H), Image.LANCZOS)


def with_caption(bg, title, sub):
    img = bg.copy()
    d = ImageDraw.Draw(img)
    d.text((W // 2, 366), title, font=font(True, 27), fill=TITLE_C, anchor="mm")
    d.text((W // 2, 402), sub, font=font(False, 18), fill=SUB_C, anchor="mm")
    return img


def add_light(base, values):
    """Additively composite lit segments plus bloom onto `base`."""
    big = Image.new("RGB", (W * SS, H * SS), (0, 0, 0))
    d = ImageDraw.Draw(big)
    for i, v in enumerate(values):
        if v > 0.01:
            v = v ** GAMMA  # perceived brightness: keeps 35% clearly dimmer than 100%
            d.polygon(seg_poly(i, SS), fill=tuple(int(c * v) for c in LIGHT))
    light = big.resize((W, H), Image.LANCZOS)

    def boost(im, k):
        return im.point([min(255, int(i * k)) for i in range(256)] * 3)

    out = ImageChops.add(base, light)
    out = ImageChops.add(out, boost(light.filter(ImageFilter.GaussianBlur(4)), 0.22))
    out = ImageChops.add(out, boost(light.filter(ImageFilter.GaussianBlur(14)), 0.40))
    out = ImageChops.add(out, boost(light.filter(ImageFilter.GaussianBlur(40)), 0.55))
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

    # One shared palette keeps colours from flickering between frames. Build it mostly from
    # the bar region, otherwise the dark background hogs the palette and mid-grey segments posterise.
    full = frames[:: max(1, len(frames) // 6)][:6]
    crops = [f.crop(BAR_BOX) for f in frames[:: max(1, len(frames) // 60)]]
    cw, ch = BAR_BOX[2] - BAR_BOX[0], BAR_BOX[3] - BAR_BOX[1]
    sheet = Image.new("RGB", (W, H * len(full) + ch * len(crops)))
    for k, f in enumerate(full):
        sheet.paste(f, (0, k * H))
    for k, c in enumerate(crops):
        sheet.paste(c, (0, H * len(full) + k * ch))
    med = sheet.quantize(colors=160, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    pal = list(med.getpalette()[: 160 * 3])
    for k in range(96):  # dedicated ramp along the unlit -> fully lit gradient: no banding on segments
        t = k / 95
        pal += [min(255, int(SEG_OFF[j] + LIGHT[j] * t)) for j in range(3)]
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
