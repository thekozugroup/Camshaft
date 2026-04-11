"""Generate a terminal-style cover image for Camshaft."""
from PIL import Image, ImageDraw, ImageFont
import json

WIDTH, HEIGHT = 1920, 1080
BG = (30, 30, 46)
FG = (205, 214, 244)
GREEN = (166, 227, 161)
BLUE = (137, 180, 250)
YELLOW = (249, 226, 175)
RED = (243, 139, 168)
MAUVE = (203, 166, 247)
TEAL = (148, 226, 213)
SURFACE = (49, 50, 68)
BAR_TOP = (69, 71, 90)

img = Image.new("RGB", (WIDTH, HEIGHT), BG)
draw = ImageDraw.Draw(img)

# Try to use a monospace font
try:
    font = ImageFont.truetype("/System/Library/Fonts/SFMono-Regular.otf", 20)
    font_bold = ImageFont.truetype("/System/Library/Fonts/SFMono-Bold.otf", 20)
    font_title = ImageFont.truetype("/System/Library/Fonts/SFMono-Bold.otf", 28)
    font_small = ImageFont.truetype("/System/Library/Fonts/SFMono-Regular.otf", 16)
except:
    try:
        font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 20)
        font_bold = font
        font_title = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 28)
        font_small = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 16)
    except:
        font = ImageFont.load_default()
        font_bold = font
        font_title = font
        font_small = font

# Terminal window chrome
draw.rectangle([(0, 0), (WIDTH, 40)], fill=BAR_TOP)
# Traffic light dots
for i, color in enumerate([(237, 106, 94), (245, 191, 79), (98, 197, 84)]):
    draw.ellipse([(20 + i * 28, 12), (36 + i * 28, 28)], fill=color)
draw.text((WIDTH // 2 - 80, 10), "camshaft", fill=FG, font=font_bold)

y = 70

# Command prompt
def draw_line(text, color=FG, y_pos=None, f=None):
    global y
    if y_pos is not None:
        y = y_pos
    draw.text((40, y), text, fill=color, font=f or font)
    y += 28

def draw_prompt(cmd):
    global y
    draw.text((40, y), "$ ", fill=GREEN, font=font_bold)
    draw.text((68, y), cmd, fill=FG, font=font)
    y += 32

# Show the optimize command and output
draw_prompt("camshaft init --name \"Auth System\" --mode sprint")
y += 4
draw_prompt("camshaft add task design-api --name \"Design API\" --duration 4")
draw_prompt("camshaft add task impl-auth --name \"Implement Auth\" --duration 8")
draw_prompt("camshaft add task write-tests --name \"Write Tests\" --duration 6")
draw_prompt("camshaft add dep design-api impl-auth && camshaft add dep design-api write-tests")
y += 8
draw_prompt("camshaft optimize")
y += 8

# JSON output with syntax highlighting
json_lines = [
    ('{', FG),
    ('  "project_duration"', BLUE, ': ', FG, '14.0', YELLOW, ',', FG),
    ('  "critical_path"', BLUE, ': ', FG, '["design-api", "impl-auth"]', GREEN, ',', FG),
    ('  "parallel_groups"', BLUE, ': [', FG, '', None),
    ('    { "group": 2, "tasks": ', FG, '["impl-auth", "write-tests"]', TEAL, ' }', FG),
    ('  ],', FG, '', None),
    ('  "suggested_order"', BLUE, ': [', FG, '', None),
    ('    "group1: design-api"', GREEN, ',', FG, '', None),
    ('    "group2: impl-auth ', MAUVE, '||', RED, ' write-tests"', MAUVE, '', None),
    ('  ]', FG, '', None),
    ('}', FG),
]

for parts in json_lines:
    x = 60
    i = 0
    while i < len(parts):
        text = parts[i]
        color = parts[i + 1] if i + 1 < len(parts) and parts[i + 1] is not None else FG
        if text:
            draw.text((x, y), text, fill=color if color else FG, font=font)
            bbox = font.getbbox(text)
            x += bbox[2] - bbox[0]
        i += 2
    y += 26

y += 20

# Bottom section: Gantt visualization
draw.rectangle([(40, y), (WIDTH - 40, y + 2)], fill=BAR_TOP)
y += 20
draw.text((40, y), "GANTT CHART", fill=MAUVE, font=font_bold)
y += 35

# Simple gantt bars
tasks = [
    ("design-api", 0, 4, GREEN, True),
    ("impl-auth", 4, 8, RED, True),
    ("write-tests", 4, 6, TEAL, False),
    ("setup-ci", 12, 2, BLUE, True),
    ("mvp", 14, 0, YELLOW, True),
]

bar_left = 250
bar_width_per_unit = 80
bar_height = 28

for name, start, dur, color, critical in tasks:
    # Label
    draw.text((40, y + 4), name, fill=FG, font=font_small)
    # Bar
    x1 = bar_left + start * bar_width_per_unit
    x2 = x1 + max(dur * bar_width_per_unit, 8)
    alpha = 255 if critical else 180
    bar_color = color if critical else tuple(int(c * 0.7) for c in color)
    draw.rectangle([(x1, y + 2), (x2, y + 2 + bar_height)], fill=bar_color)
    if dur > 0:
        dur_text = f"{dur}h"
        draw.text((x1 + 8, y + 6), dur_text, fill=BG, font=font_small)
    else:
        # Milestone diamond
        mx = x1
        draw.polygon([(mx, y+16), (mx+10, y+6), (mx+20, y+16), (mx+10, y+26)], fill=YELLOW)
    # Critical path indicator
    if critical and dur > 0:
        draw.text((x2 + 8, y + 6), "*", fill=RED, font=font_small)
    y += 38

# Timeline markers
y_timeline = y
for i in range(0, 16, 2):
    x = bar_left + i * bar_width_per_unit
    draw.text((x, y_timeline), f"{i}", fill=MAUVE, font=font_small)

# Bottom branding
draw.text((40, HEIGHT - 40), "camshaft v0.1.0", fill=BAR_TOP, font=font_small)
draw.text((WIDTH - 350, HEIGHT - 40), "Powered by GanttML", fill=BAR_TOP, font=font_small)

img.save("/Users/michaelwong/Developer/Camshaft/docs/screenshot.png", "PNG")
print(f"Generated: {WIDTH}x{HEIGHT} PNG")
