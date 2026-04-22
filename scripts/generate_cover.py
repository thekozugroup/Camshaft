"""Generate an abstract, octane-render-style Gantt chart cover for Camshaft.

Renders stylized bars with gradient fills, soft glows, drop shadows, depth
parallax, and a volumetric background — no real data, just a hero visual.
"""
from __future__ import annotations

import math
import random
from PIL import Image, ImageDraw, ImageFilter, ImageFont

WIDTH, HEIGHT = 1920, 1080
OUT_PATH = "/Users/michaelwong/Developer/Camshaft/docs/screenshot.png"

# Palette — deep space gradient + neon accents
BG_TOP = (8, 10, 28)
BG_MID = (18, 12, 46)
BG_BOT = (32, 18, 64)

ACCENT_PINK = (255, 82, 163)
ACCENT_CYAN = (68, 226, 255)
ACCENT_VIOLET = (148, 108, 255)
ACCENT_GOLD = (255, 196, 84)
ACCENT_GREEN = (120, 255, 176)

GRID = (58, 48, 110)
TEXT_MUTED = (160, 170, 230)
TEXT = (220, 225, 245)

random.seed(7)


def load_font(size: int, bold: bool = False):
    candidates = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFNSMono.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/Menlo.ttc",
    ]
    for p in candidates:
        try:
            return ImageFont.truetype(p, size, index=1 if bold else 0)
        except Exception:
            continue
    return ImageFont.load_default()


def vertical_gradient(w: int, h: int, stops: list[tuple[float, tuple[int, int, int]]]) -> Image.Image:
    """Build a vertical gradient from (position, rgb) stops."""
    img = Image.new("RGB", (w, h), stops[0][1])
    px = img.load()
    stops = sorted(stops)
    for y in range(h):
        t = y / (h - 1)
        # Find bounding stops
        for i in range(len(stops) - 1):
            p0, c0 = stops[i]
            p1, c1 = stops[i + 1]
            if p0 <= t <= p1:
                span = max(p1 - p0, 1e-6)
                k = (t - p0) / span
                r = int(c0[0] + (c1[0] - c0[0]) * k)
                g = int(c0[1] + (c1[1] - c0[1]) * k)
                b = int(c0[2] + (c1[2] - c0[2]) * k)
                for x in range(w):
                    px[x, y] = (r, g, b)
                break
    return img


def radial_glow(w: int, h: int, cx: int, cy: int, radius: int, color: tuple[int, int, int], strength: float = 1.0) -> Image.Image:
    """Soft radial light blob in RGBA, meant to be pasted with .alpha_composite."""
    layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    steps = 48
    for i in range(steps, 0, -1):
        t = i / steps
        rr = int(radius * t)
        alpha = int(190 * (1 - t) ** 2.2 * strength)
        draw.ellipse(
            [cx - rr, cy - rr, cx + rr, cy + rr],
            fill=(color[0], color[1], color[2], alpha),
        )
    return layer.filter(ImageFilter.GaussianBlur(radius=radius // 6))


def rounded_bar(width: int, height: int, color_a: tuple[int, int, int], color_b: tuple[int, int, int], radius: int = 22) -> Image.Image:
    """A bar with horizontal gradient, rounded corners, specular highlight, alpha mask."""
    bar = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    # Base gradient
    base = Image.new("RGB", (width, height), color_a)
    px = base.load()
    for x in range(width):
        t = x / max(width - 1, 1)
        r = int(color_a[0] + (color_b[0] - color_a[0]) * t)
        g = int(color_a[1] + (color_b[1] - color_a[1]) * t)
        b = int(color_a[2] + (color_b[2] - color_a[2]) * t)
        for y in range(height):
            # Subtle vertical shading for depth
            sh = 1.0 - (abs(y - height / 2) / (height / 2)) * 0.18
            px[x, y] = (int(r * sh), int(g * sh), int(b * sh))

    # Rounded alpha mask
    mask = Image.new("L", (width, height), 0)
    mdraw = ImageDraw.Draw(mask)
    mdraw.rounded_rectangle([0, 0, width - 1, height - 1], radius=radius, fill=255)
    bar.paste(base, (0, 0), mask)

    # Specular top highlight
    spec = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    sdraw = ImageDraw.Draw(spec)
    sdraw.rounded_rectangle(
        [4, 3, width - 5, height // 3],
        radius=radius // 2,
        fill=(255, 255, 255, 42),
    )
    spec = spec.filter(ImageFilter.GaussianBlur(radius=6))
    bar = Image.alpha_composite(bar, spec)

    # Inner glow edge
    edge = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    edraw = ImageDraw.Draw(edge)
    edraw.rounded_rectangle([1, 1, width - 2, height - 2], radius=radius, outline=(255, 255, 255, 55), width=1)
    bar = Image.alpha_composite(bar, edge)

    return bar


def paste_with_glow(canvas: Image.Image, bar: Image.Image, x: int, y: int, glow_color: tuple[int, int, int]) -> None:
    """Composite a bar onto canvas with an outer glow + drop shadow."""
    w, h = bar.size
    # Drop shadow
    shadow = Image.new("RGBA", (w + 80, h + 80), (0, 0, 0, 0))
    sdraw = ImageDraw.Draw(shadow)
    sdraw.rounded_rectangle([40, 40, 40 + w, 40 + h], radius=22, fill=(0, 0, 0, 180))
    shadow = shadow.filter(ImageFilter.GaussianBlur(radius=22))
    canvas.alpha_composite(shadow, (x - 40, y - 20))

    # Outer color glow
    glow = Image.new("RGBA", (w + 120, h + 120), (0, 0, 0, 0))
    gdraw = ImageDraw.Draw(glow)
    gdraw.rounded_rectangle(
        [60, 60, 60 + w, 60 + h], radius=32, fill=(*glow_color, 120)
    )
    glow = glow.filter(ImageFilter.GaussianBlur(radius=36))
    canvas.alpha_composite(glow, (x - 60, y - 60))

    # Bar itself
    canvas.alpha_composite(bar, (x, y))


def draw_grid(canvas: Image.Image, left: int, top: int, right: int, bottom: int, rows: int, cols: int) -> None:
    overlay = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)
    col_step = (right - left) / cols
    row_step = (bottom - top) / rows

    # Horizontal lines
    for i in range(rows + 1):
        y = int(top + i * row_step)
        draw.line([(left, y), (right, y)], fill=(*GRID, 60), width=1)

    # Vertical time ticks
    for j in range(cols + 1):
        x = int(left + j * col_step)
        alpha = 120 if j % 4 == 0 else 45
        draw.line([(x, top), (x, bottom)], fill=(*GRID, alpha), width=1 if j % 4 else 2)

    canvas.alpha_composite(overlay)


def draw_connection(canvas: Image.Image, start: tuple[int, int], end: tuple[int, int], color: tuple[int, int, int]) -> None:
    """Neon bezier-ish connector between two task bars."""
    overlay = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)

    x0, y0 = start
    x1, y1 = end
    # Cubic bezier sampled as polyline
    points = []
    for step in range(0, 101):
        t = step / 100
        cx0 = x0 + (x1 - x0) * 0.45
        cy0 = y0
        cx1 = x0 + (x1 - x0) * 0.55
        cy1 = y1
        bx = (1 - t) ** 3 * x0 + 3 * (1 - t) ** 2 * t * cx0 + 3 * (1 - t) * t ** 2 * cx1 + t ** 3 * x1
        by = (1 - t) ** 3 * y0 + 3 * (1 - t) ** 2 * t * cy0 + 3 * (1 - t) * t ** 2 * cy1 + t ** 3 * y1
        points.append((bx, by))

    # Multiple strokes for glow
    for width, alpha in [(10, 45), (6, 85), (3, 170), (1, 240)]:
        for i in range(len(points) - 1):
            draw.line([points[i], points[i + 1]], fill=(*color, alpha), width=width)

    # Soft blur on wide strokes
    canvas.alpha_composite(overlay.filter(ImageFilter.GaussianBlur(radius=1)))

    # End node
    dot_layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    ddraw = ImageDraw.Draw(dot_layer)
    ddraw.ellipse([x1 - 8, y1 - 8, x1 + 8, y1 + 8], fill=(*color, 255))
    ddraw.ellipse([x1 - 14, y1 - 14, x1 + 14, y1 + 14], outline=(*color, 120), width=2)
    canvas.alpha_composite(dot_layer)


def draw_particles(canvas: Image.Image, count: int = 140) -> None:
    layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    for _ in range(count):
        x = random.randint(0, WIDTH)
        y = random.randint(0, HEIGHT)
        r = random.choice([1, 1, 1, 2, 2, 3, 4])
        alpha = random.randint(40, 180)
        palette = random.choice([ACCENT_CYAN, ACCENT_VIOLET, ACCENT_PINK, (220, 220, 240)])
        draw.ellipse([x - r, y - r, x + r, y + r], fill=(*palette, alpha))
    layer = layer.filter(ImageFilter.GaussianBlur(radius=0.6))
    canvas.alpha_composite(layer)


def draw_noise(canvas: Image.Image, strength: float = 12) -> None:
    noise = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    px = noise.load()
    for _ in range(8000):
        x = random.randint(0, WIDTH - 1)
        y = random.randint(0, HEIGHT - 1)
        v = random.randint(0, int(strength))
        px[x, y] = (v, v, v, 60)
    canvas.alpha_composite(noise)


def main() -> None:
    # 1. Background gradient
    bg = vertical_gradient(WIDTH, HEIGHT, [(0.0, BG_TOP), (0.45, BG_MID), (1.0, BG_BOT)])
    canvas = bg.convert("RGBA")

    # 2. Volumetric glows
    for cx, cy, r, col in [
        (260, 180, 620, ACCENT_VIOLET),
        (1720, 240, 520, ACCENT_CYAN),
        (1550, 900, 620, ACCENT_PINK),
        (360, 860, 420, ACCENT_GOLD),
    ]:
        canvas.alpha_composite(radial_glow(WIDTH, HEIGHT, cx, cy, r, col, strength=0.7))

    # 3. Back-plate panel (glassy card vibes)
    panel = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
    pdraw = ImageDraw.Draw(panel)
    pdraw.rounded_rectangle(
        [90, 110, WIDTH - 90, HEIGHT - 110],
        radius=36,
        fill=(14, 16, 42, 200),
        outline=(120, 140, 220, 40),
        width=2,
    )
    panel = panel.filter(ImageFilter.GaussianBlur(radius=0.4))
    canvas.alpha_composite(panel)

    # 4. Grid
    draw_grid(canvas, left=260, top=230, right=WIDTH - 150, bottom=HEIGHT - 220, rows=6, cols=24)

    # 5. Task bars — fake Gantt layout
    #    (row_idx, start_col, span_col, color_pair, label)
    rows = [
        (0, 0, 6, (ACCENT_VIOLET, ACCENT_PINK), "DISCOVERY"),
        (1, 4, 8, (ACCENT_CYAN, ACCENT_VIOLET), "DESIGN SCHEMA"),
        (2, 7, 6, (ACCENT_PINK, ACCENT_GOLD), "CORE IMPL"),
        (2, 13, 5, (ACCENT_GREEN, ACCENT_CYAN), "TESTS"),
        (3, 11, 7, (ACCENT_VIOLET, ACCENT_CYAN), "INTEGRATION"),
        (4, 16, 5, (ACCENT_GOLD, ACCENT_PINK), "REVIEW"),
        (5, 19, 4, (ACCENT_CYAN, ACCENT_GREEN), "SHIP"),
    ]

    grid_left = 260
    grid_right = WIDTH - 150
    grid_top = 230
    grid_bottom = HEIGHT - 220
    row_step = (grid_bottom - grid_top) / 6
    col_step = (grid_right - grid_left) / 24

    bar_centers: dict[str, tuple[int, int, int]] = {}  # label -> (left_x, right_x, center_y)

    for row_idx, start, span, (c1, c2), label in rows:
        x = int(grid_left + start * col_step) + 8
        y = int(grid_top + row_idx * row_step) + int(row_step * 0.18)
        w = int(span * col_step) - 16
        h = int(row_step * 0.64)

        bar = rounded_bar(w, h, c1, c2, radius=h // 2)
        paste_with_glow(canvas, bar, x, y, glow_color=c2)
        bar_centers[label] = (x, x + w, y + h // 2)

    # 6. Connection lines between key milestones (critical-path flavor)
    draw = ImageDraw.Draw(canvas)
    connections = [
        ("DISCOVERY", "DESIGN SCHEMA", ACCENT_CYAN),
        ("DESIGN SCHEMA", "CORE IMPL", ACCENT_PINK),
        ("CORE IMPL", "INTEGRATION", ACCENT_VIOLET),
        ("INTEGRATION", "REVIEW", ACCENT_GOLD),
        ("REVIEW", "SHIP", ACCENT_GREEN),
    ]
    for a, b, col in connections:
        if a in bar_centers and b in bar_centers:
            _, ax_r, ay = bar_centers[a]
            bx_l, _, by = bar_centers[b]
            draw_connection(canvas, (ax_r, ay), (bx_l, by), col)

    # 7. Row labels
    label_font = load_font(22, bold=True)
    for row_idx, _, _, _, label in rows:
        y = int(grid_top + row_idx * row_step) + int(row_step * 0.42)
        draw.text((110, y), label, fill=TEXT_MUTED, font=label_font)

    # 8. Time axis ticks
    tick_font = load_font(18)
    for j in range(0, 25, 4):
        x = int(grid_left + j * col_step)
        draw.text((x - 8, grid_bottom + 14), f"T+{j}", fill=TEXT_MUTED, font=tick_font)

    # 9. Header
    title_font = load_font(72, bold=True)
    subtitle_font = load_font(26)
    draw.text((110, 30), "CAMSHAFT", fill=TEXT, font=title_font)
    draw.text(
        (112, 110),
        "Dependency-aware planning for AI code agents",
        fill=TEXT_MUTED,
        font=subtitle_font,
    )

    # Top-right meta
    meta_font = load_font(20, bold=True)
    draw.text(
        (WIDTH - 460, 42),
        "CRITICAL PATH ACTIVE",
        fill=ACCENT_GREEN,
        font=meta_font,
    )
    draw.text(
        (WIDTH - 460, 72),
        "18 commands · 45 tests · Rust + GanttML",
        fill=TEXT_MUTED,
        font=load_font(18),
    )

    # 10. Stars / particle dust
    draw_particles(canvas, count=220)

    # 11. Subtle noise for photoreal grain
    draw_noise(canvas)

    # 12. Vignette
    vignette = Image.new("L", (WIDTH, HEIGHT), 0)
    vd = ImageDraw.Draw(vignette)
    vd.ellipse([-400, -400, WIDTH + 400, HEIGHT + 400], fill=255)
    vignette = vignette.filter(ImageFilter.GaussianBlur(radius=160))
    mask = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    mdraw = ImageDraw.Draw(mask)
    mdraw.rectangle([0, 0, WIDTH, HEIGHT], fill=(0, 0, 0, 90))
    canvas.alpha_composite(mask)

    # 13. Save as PNG (convert to RGB for smaller file)
    final = canvas.convert("RGB")
    final.save(OUT_PATH, "PNG", optimize=True)
    import os

    size_kb = os.path.getsize(OUT_PATH) / 1024
    print(f"Saved: {OUT_PATH} ({WIDTH}x{HEIGHT}, {size_kb:.1f} KB)")


if __name__ == "__main__":
    main()
