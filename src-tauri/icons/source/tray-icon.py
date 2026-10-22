"""Draws src-tauri/icons/tray-icon-template.png, the menu bar glyph: the
app icon's prompt, wave and cursor as plain strokes. Run from the repo
root: python3 src-tauri/icons/source/tray-icon.py (needs Pillow). Also
writes a preview at real menu bar sizes to /tmp/tray-preview.png."""
import math
from PIL import Image, ImageDraw

S = 1024          # height; drawn big, downsampled for smooth edges
WIDTH = 1408      # wider than tall, like the glyph
W = 118           # stroke width

def stroke(d, pts):
    d.line(pts, fill=(0, 0, 0, 255), width=W, joint="curve")
    r = W / 2
    for x, y in (pts[0], pts[-1]):
        d.ellipse([x - r, y - r, x + r, y + r], fill=(0, 0, 0, 255))

img = Image.new("RGBA", (WIDTH, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
cy = S / 2
# ">" prompt
stroke(d, [(110, 230), (370, cy), (110, 794)])
# one soft wave: a single full period, tall in the middle like the app icon
x0, x1, amp = 500, 1060, 300
pts = []
for i in range(121):
    t = i / 120
    env = math.sin(math.pi * t)            # fades in and out at the ends
    pts.append((x0 + (x1 - x0) * t, cy - amp * env * math.sin(2 * math.pi * t)))
stroke(d, pts)
# "|" cursor
stroke(d, [(1260, 160), (1260, 864)])
img.resize((176, 128), Image.LANCZOS).save("src-tauri/icons/tray-icon-template.png")

# Previews: real menu bar sizes on light and dark bars (template images are
# tinted by macOS: black on light, white on dark).
icon = Image.open("src-tauri/icons/tray-icon-template.png")
row = Image.new("RGBA", (420, 120), (0, 0, 0, 0))
for i, (bg, tint) in enumerate([((236, 236, 236, 255), (0, 0, 0)), ((40, 40, 44, 255), (255, 255, 255))]):
    band = Image.new("RGBA", (420, 60), bg)
    x = 12
    for px in (16, 18, 22, 36, 44):
        sm = icon.resize((round(px * 176 / 128), px), Image.LANCZOS)
        colored = Image.new("RGBA", sm.size, tint + (255,))
        colored.putalpha(sm.getchannel("A"))
        band.alpha_composite(colored, (x, (60 - px) // 2))
        x += round(px * 176 / 128) + 24
    row.alpha_composite(band, (0, i * 60))
row = row.resize((840, 240), Image.NEAREST)
row.save("/tmp/tray-preview.png")
