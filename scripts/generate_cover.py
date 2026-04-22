"""Abstract, minimal hero visual for Camshaft.

Kimi-blog style: near-black background, single luminous composition,
iridescent rim lighting, massive negative space. The subject is a set of
flowing horizontal light streaks — a Gantt chart reduced to pure essence.
"""
from __future__ import annotations

import math
import random
from PIL import Image, ImageDraw, ImageFilter

WIDTH, HEIGHT = 1920, 1080
OUT_PATH = "/Users/michaelwong/Developer/Camshaft/docs/screenshot.png"

BG = (4, 4, 8)

# Iridescent dispersion palette — soft jewel tones, low saturation
COOL = (120, 170, 255)    # soft cyan-blue
VIOLET = (170, 140, 255)  # periwinkle violet
MAGENTA = (230, 140, 200) # soft pink
AMBER = (255, 180, 120)   # warm amber
WARM = (255, 140, 130)    # soft coral

random.seed(11)


def solid_bg() -> Image.Image:
    """Near-black background with a very subtle center vignette."""
    img = Image.new("RGB", (WIDTH, HEIGHT), BG)
    # Extremely subtle radial lift near center so pure black feels alive
    lift = Image.new("L", (WIDTH, HEIGHT), 0)
    d = ImageDraw.Draw(lift)
    d.ellipse(
        [WIDTH // 2 - 1100, HEIGHT // 2 - 700, WIDTH // 2 + 1100, HEIGHT // 2 + 700],
        fill=18,
    )
    lift = lift.filter(ImageFilter.GaussianBlur(radius=260))
    base = img.convert("RGBA")
    tint = Image.new("RGBA", (WIDTH, HEIGHT), (40, 44, 80, 0))
    tint.putalpha(lift)
    base.alpha_composite(tint)
    return base


def streak(width: int, height: int, color_stops: list[tuple[float, tuple[int, int, int]]]) -> Image.Image:
    """A single glowing horizontal streak with horizontal gradient + soft rounded ends + bloom."""
    # Core gradient band
    core = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    px = core.load()
    stops = sorted(color_stops)
    for x in range(width):
        t = x / max(width - 1, 1)
        # Find bracketing stops
        c = stops[0][1]
        for i in range(len(stops) - 1):
            p0, c0 = stops[i]
            p1, c1 = stops[i + 1]
            if p0 <= t <= p1:
                span = max(p1 - p0, 1e-6)
                k = (t - p0) / span
                c = (
                    int(c0[0] + (c1[0] - c0[0]) * k),
                    int(c0[1] + (c1[1] - c0[1]) * k),
                    int(c0[2] + (c1[2] - c0[2]) * k),
                )
                break

        # Vertical alpha profile — soft bell so the streak feathers into the void
        for y in range(height):
            ny = (y - (height - 1) / 2) / (height / 2)
            # Feathered profile: bell shape to the power of 1.5 — softer edges
            vert = max(0.0, 1.0 - abs(ny) ** 1.5)
            # Horizontal end-feather — fade first/last 8 percent
            if t < 0.08:
                end = t / 0.08
            elif t > 0.92:
                end = (1.0 - t) / 0.08
            else:
                end = 1.0
            alpha = int(255 * vert * end)
            if alpha <= 0:
                continue
            px[x, y] = (c[0], c[1], c[2], alpha)
    return core


def paste_streak_with_bloom(canvas: Image.Image, streak_img: Image.Image, x: int, y: int, bloom_color: tuple[int, int, int]) -> None:
    """Paste a streak plus a wide soft color bloom behind it."""
    w, h = streak_img.size

    # Wide bloom — blurred color halo
    halo_w, halo_h = w + 500, h + 320
    halo = Image.new("RGBA", (halo_w, halo_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(halo)
    d.ellipse(
        [60, halo_h // 2 - h, halo_w - 60, halo_h // 2 + h],
        fill=(*bloom_color, 60),
    )
    halo = halo.filter(ImageFilter.GaussianBlur(radius=90))
    canvas.alpha_composite(halo, (x - (halo_w - w) // 2, y - (halo_h - h) // 2))

    # Medium bloom — tighter, brighter
    mid_w, mid_h = w + 200, h + 120
    mid = Image.new("RGBA", (mid_w, mid_h), (0, 0, 0, 0))
    d2 = ImageDraw.Draw(mid)
    d2.ellipse(
        [40, mid_h // 2 - h // 2, mid_w - 40, mid_h // 2 + h // 2],
        fill=(*bloom_color, 95),
    )
    mid = mid.filter(ImageFilter.GaussianBlur(radius=38))
    canvas.alpha_composite(mid, (x - (mid_w - w) // 2, y - (mid_h - h) // 2))

    # The streak itself
    canvas.alpha_composite(streak_img, (x, y))

    # Subtle specular sheen on top edge
    sheen = Image.new("RGBA", (w, h // 2), (0, 0, 0, 0))
    sd = ImageDraw.Draw(sheen)
    sd.rectangle([0, 0, w, h // 2], fill=(255, 255, 255, 22))
    mask = Image.new("L", (w, h // 2), 0)
    md = ImageDraw.Draw(mask)
    md.ellipse([-80, 0, w + 80, h], fill=240)
    sheen.putalpha(mask)
    sheen = sheen.filter(ImageFilter.GaussianBlur(radius=14))
    canvas.alpha_composite(sheen, (x, y))


def chromatic_rim(canvas: Image.Image, cx: int, cy: int, length: int, thickness: int) -> None:
    """Paint a horizontal rim of chromatic dispersion — cool side and warm side bleed."""
    # Cool side — top
    cool = Image.new("RGBA", (length + 400, 240), (0, 0, 0, 0))
    d = ImageDraw.Draw(cool)
    d.ellipse([0, 40, length + 400, 200], fill=(*COOL, 55))
    cool = cool.filter(ImageFilter.GaussianBlur(radius=80))
    canvas.alpha_composite(cool, (cx - (length + 400) // 2, cy - 220))

    # Warm side — bottom
    warm = Image.new("RGBA", (length + 400, 240), (0, 0, 0, 0))
    d = ImageDraw.Draw(warm)
    d.ellipse([0, 40, length + 400, 200], fill=(*WARM, 45))
    warm = warm.filter(ImageFilter.GaussianBlur(radius=80))
    canvas.alpha_composite(warm, (cx - (length + 400) // 2, cy - 20))


def particle_dust(canvas: Image.Image, count: int = 70) -> None:
    """Minimal star/particle dust."""
    layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    for _ in range(count):
        x = random.randint(0, WIDTH)
        y = random.randint(0, HEIGHT)
        r = random.choice([1, 1, 1, 2])
        alpha = random.randint(20, 90)
        d.ellipse([x - r, y - r, x + r, y + r], fill=(240, 240, 255, alpha))
    layer = layer.filter(ImageFilter.GaussianBlur(radius=0.4))
    canvas.alpha_composite(layer)


def film_grain(canvas: Image.Image, amount: int = 4200) -> None:
    """A touch of monochrome grain so it reads as photographic render, not digital."""
    noise = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    px = noise.load()
    for _ in range(amount):
        x = random.randint(0, WIDTH - 1)
        y = random.randint(0, HEIGHT - 1)
        v = random.randint(0, 30)
        a = random.randint(20, 55)
        px[x, y] = (v, v, v, a)
    canvas.alpha_composite(noise)


def main() -> None:
    canvas = solid_bg()

    # Composition: 4 streaks, staggered horizontally and vertically.
    # Each has its own iridescent gradient. Together they feel like a
    # long-exposure photograph of parallel scheduling beams.
    center_y = HEIGHT // 2
    streak_height = 18  # thin, minimal
    streaks = [
        # (start_x_frac, end_x_frac, y_offset, gradient, bloom)
        (0.12, 0.58, -220, [(0.0, COOL), (0.5, VIOLET), (1.0, MAGENTA)], VIOLET),
        (0.28, 0.82, -70, [(0.0, VIOLET), (0.5, MAGENTA), (1.0, AMBER)], MAGENTA),
        (0.18, 0.72, 80, [(0.0, COOL), (0.5, COOL), (1.0, VIOLET)], COOL),
        (0.40, 0.88, 230, [(0.0, MAGENTA), (0.5, WARM), (1.0, AMBER)], WARM),
    ]

    # Background atmospheric rim — one big soft horizontal light band
    rim_layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(rim_layer)
    d.ellipse(
        [WIDTH // 2 - 900, center_y - 80, WIDTH // 2 + 900, center_y + 80],
        fill=(*COOL, 35),
    )
    d.ellipse(
        [WIDTH // 2 - 700, center_y + 80, WIDTH // 2 + 700, center_y + 220],
        fill=(*WARM, 28),
    )
    rim_layer = rim_layer.filter(ImageFilter.GaussianBlur(radius=140))
    canvas.alpha_composite(rim_layer)

    # Paint streaks back-to-front
    for sx, ex, dy, grad, bloom in streaks:
        x = int(WIDTH * sx)
        end = int(WIDTH * ex)
        w = end - x
        h = streak_height
        s = streak(w, h, grad)
        paste_streak_with_bloom(canvas, s, x, center_y + dy - h // 2, bloom)

    # Chromatic rim sweep across the whole composition
    chromatic_rim(canvas, WIDTH // 2, center_y, length=1500, thickness=300)

    # A single bright specular highlight disk — echo of rim-lit sphere
    highlight = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(highlight)
    d.ellipse(
        [WIDTH // 2 + 200, center_y - 40, WIDTH // 2 + 340, center_y + 40],
        fill=(255, 245, 230, 80),
    )
    highlight = highlight.filter(ImageFilter.GaussianBlur(radius=40))
    canvas.alpha_composite(highlight)

    # Minimal dust + grain
    particle_dust(canvas, count=60)
    film_grain(canvas, amount=5200)

    # Final: soft overall vignette
    vignette_mask = Image.new("L", (WIDTH, HEIGHT), 0)
    vd = ImageDraw.Draw(vignette_mask)
    vd.ellipse([-500, -500, WIDTH + 500, HEIGHT + 500], fill=255)
    vignette_mask = vignette_mask.filter(ImageFilter.GaussianBlur(radius=220))
    dark = Image.new("RGBA", canvas.size, (0, 0, 0, 140))
    inv = Image.eval(vignette_mask, lambda v: 255 - v)
    dark.putalpha(inv)
    canvas.alpha_composite(dark)

    final = canvas.convert("RGB")
    final.save(OUT_PATH, "PNG", optimize=True)
    import os

    size_kb = os.path.getsize(OUT_PATH) / 1024
    print(f"Saved: {OUT_PATH} ({WIDTH}x{HEIGHT}, {size_kb:.1f} KB)")


if __name__ == "__main__":
    main()
