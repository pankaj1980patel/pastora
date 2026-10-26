"""Generates the app icon (assets/icon.png) and the menu bar template icon."""
from PIL import Image, ImageDraw

def clipboard(draw, s, fg, board_fill=None, lock=False):
    # board
    bx0, by0, bx1, by1 = s*0.24, s*0.20, s*0.76, s*0.84
    draw.rounded_rectangle([bx0, by0, bx1, by1], radius=s*0.07, outline=fg, width=max(2, int(s*0.05)), fill=board_fill)
    # clip
    draw.rounded_rectangle([s*0.38, s*0.13, s*0.62, s*0.27], radius=s*0.04, fill=fg)
    # lines
    w = max(2, int(s*0.045))
    for i, y in enumerate((0.42, 0.54, 0.66)):
        x1 = 0.66 if i < 2 else 0.54
        draw.line([s*0.34, s*y, s*x1, s*y], fill=fg, width=w)

# App icon 1024
S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
grad = Image.new("RGBA", (S, S))
gd = ImageDraw.Draw(grad)
for y in range(S):
    t = y / S
    c = (int(99 + (139-99)*t), int(102 + (92-102)*t), int(241 + (246-241)*t), 255)
    gd.line([0, y, S, y], fill=c)
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([80, 80, S-80, S-80], radius=200, fill=255)
img.paste(grad, (0, 0), mask)
clipboard(ImageDraw.Draw(img), S, (255, 255, 255, 255))
img.save("assets/icon.png")

# Tray template icon (black on transparent), 1x and 2x
for size, name in ((22, "tray.png"), (44, "tray@2x.png")):
    big = Image.new("RGBA", (size*8, size*8), (0, 0, 0, 0))
    clipboard(ImageDraw.Draw(big), size*8, (0, 0, 0, 255))
    big.resize((size, size), Image.LANCZOS).save(f"src-tauri/icons/{name}")
print("ok")
