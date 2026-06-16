"""Воссоздание официального логотипа vrox.vpn по дизайну из
claude.ai/design (logo-export.html): знак V из двух "лезвий"
(back blade alpha 0.4, front blade alpha 1.0), Path2D-координаты
из оригинального canvas-кода сохранены как есть."""
import math
from PIL import Image, ImageDraw

OUT_DIR = "/home/beardrubyblue/Документы/Vpn/vroxory-vpn/assets/icons"
ICON_NAME = "com.vroxory.vpn"

BG_COLOR = (10, 10, 10)       # #0a0a0a — тёмная подложка из дизайна
WHITE = (255, 255, 255)
UNIT_LINE_WIDTH = 2.5          # из оригинала: ctx.lineWidth = 2.5 (в 32-юнитах)

# пути из оригинального JS (M x y L x y L x y), в 32-юнитной системе
BACK_BLADE = [(10, 6), (16, 16), (22, 6)]   # alpha 0.4
FRONT_BLADE = [(4, 6), (16, 26), (28, 6)]   # alpha 1.0
ART_UNIT = 32  # координатная плоскость знака


def blend(fg, bg, alpha):
    return tuple(round(fg[i] * alpha + bg[i] * (1 - alpha)) for i in range(3))


def draw_stroke(draw, points, scale, ox, oy, color, width_px):
    pts = [(ox + x * scale, oy + y * scale) for x, y in points]
    draw.line(pts, fill=color, width=round(width_px), joint="curve")
    r = width_px / 2
    for p in pts:
        draw.ellipse([p[0] - r, p[1] - r, p[0] + r, p[1] + r], fill=color)


def make_icon(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    radius = size * 0.22
    draw.rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=BG_COLOR + (255,))

    # знак занимает ~60% площади иконки, по центру
    box = size * 0.62
    scale = box / ART_UNIT
    ox = (size - ART_UNIT * scale) / 2
    oy = (size - ART_UNIT * scale) / 2

    line_w = max(UNIT_LINE_WIDTH * scale, size * 0.045)
    back_color = blend(WHITE, BG_COLOR, 0.4)

    draw_stroke(draw, BACK_BLADE, scale, ox, oy, back_color, line_w)
    draw_stroke(draw, FRONT_BLADE, scale, ox, oy, WHITE, line_w)

    return img


def make_wordmark(height: int) -> Image.Image:
    """mark + 'vrox' (белый) + '.vpn' (40% альфа) — для README/баннеров."""
    from PIL import ImageFont

    scale = height / 48
    mark_unit = 36 * scale
    gap = 10 * scale
    pad_x = 14 * scale
    pad_y = 10 * scale
    font_px = round(32 * scale)

    try:
        font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", font_px)
    except OSError:
        font = ImageFont.load_default()

    tmp = Image.new("RGBA", (10, 10))
    tmp_draw = ImageDraw.Draw(tmp)
    w_full = tmp_draw.textlength("vrox.vpn", font=font)
    w_vrox = tmp_draw.textlength("vrox", font=font)

    width = pad_x + mark_unit + gap + w_full + pad_x
    img = Image.new("RGBA", (round(width), height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.rounded_rectangle([0, 0, width - 1, height - 1], radius=height * 0.22, fill=BG_COLOR + (255,))

    art_scale = mark_unit / ART_UNIT
    ox = pad_x + (mark_unit - ART_UNIT * art_scale) / 2
    oy = (height - ART_UNIT * art_scale) / 2
    line_w = UNIT_LINE_WIDTH * art_scale
    back_color = blend(WHITE, BG_COLOR, 0.4)
    draw_stroke(draw, BACK_BLADE, art_scale, ox, oy, back_color, line_w)
    draw_stroke(draw, FRONT_BLADE, art_scale, ox, oy, WHITE, line_w)

    tx = pad_x + mark_unit + gap
    ty = height / 2
    draw.text((tx, ty), "vrox", font=font, fill=WHITE, anchor="lm")
    draw.text((tx + w_vrox, ty), ".vpn", font=font, fill=blend(WHITE, BG_COLOR, 0.4), anchor="lm")

    return img


def main():
    for size in (16, 32, 48, 64, 128, 256, 512):
        make_icon(size).save(f"{OUT_DIR}/{ICON_NAME}-{size}.png")
        print(f"saved {ICON_NAME}-{size}.png")
    make_wordmark(96).save(f"{OUT_DIR}/wordmark.png")
    print("saved wordmark.png")


if __name__ == "__main__":
    main()
