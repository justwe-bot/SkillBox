import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw


BACKGROUND = (6, 6, 26, 255)
FOREGROUND = (255, 255, 255, 255)
RENDER_SCALE = 8
BACKGROUND_PAD_RATIO = 0.035
BACKGROUND_RADIUS_RATIO = 0.24


def polygon(points, scale, offset_x, offset_y):
    return [(offset_x + (x * scale), offset_y + (y * scale)) for x, y in points]


def draw_logo(draw: ImageDraw.ImageDraw, scale: float, offset_x: float, offset_y: float) -> None:
    draw.polygon(
        polygon([(4, 13), (8, 11), (12, 13), (8, 15)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.8)),
    )
    draw.polygon(
        polygon([(4, 13), (4, 17), (8, 19), (8, 15)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.5)),
    )
    draw.polygon(
        polygon([(8, 15), (8, 19), (12, 17), (12, 13)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.6)),
    )

    draw.polygon(
        polygon([(12, 13), (16, 11), (20, 13), (16, 15)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.8)),
    )
    draw.polygon(
        polygon([(12, 13), (12, 17), (16, 19), (16, 15)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.5)),
    )
    draw.polygon(
        polygon([(16, 15), (16, 19), (20, 17), (20, 13)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.6)),
    )

    draw.polygon(
        polygon([(8, 7), (12, 5), (16, 7), (12, 9)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.9)),
    )
    draw.polygon(
        polygon([(8, 7), (8, 11), (12, 13), (12, 9)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.6)),
    )
    draw.polygon(
        polygon([(12, 9), (12, 13), (16, 11), (16, 7)], scale, offset_x, offset_y),
        fill=(255, 255, 255, round(255 * 0.7)),
    )

    line_points = polygon([(8, 11), (12, 13), (16, 11)], scale, offset_x, offset_y)
    draw.line(line_points, fill=(255, 255, 255, round(255 * 0.4)), width=max(1, round(scale * 0.28)))


def create_icon(size: int) -> Image.Image:
    render_size = size * RENDER_SCALE
    base = Image.new("RGBA", (render_size, render_size), (0, 0, 0, 0))
    base_draw = ImageDraw.Draw(base, "RGBA")
    logo_layer = Image.new("RGBA", (render_size, render_size), (0, 0, 0, 0))
    logo_draw = ImageDraw.Draw(logo_layer, "RGBA")

    # Keep the icon nearly full-bleed so macOS bundle icons match the larger
    # appearance we see during development instead of looking inset on a plate.
    pad = max(1, round(render_size * BACKGROUND_PAD_RATIO))
    radius = round(render_size * BACKGROUND_RADIUS_RATIO)

    base_draw.rounded_rectangle(
        [pad, pad, render_size - pad, render_size - pad],
        radius=radius,
        fill=BACKGROUND,
    )

    logo_scale = render_size / 112 * 3.95
    offset_x = (render_size / 2) - (12 * logo_scale)
    offset_y = (render_size / 2) - (12 * logo_scale)
    draw_logo(logo_draw, logo_scale, offset_x, offset_y)

    composed = Image.alpha_composite(base, logo_layer)
    return composed.resize((size, size), Image.Resampling.LANCZOS)


def build_icns(base: Path) -> None:
    iconset = base / "icon.iconset"
    if iconset.exists():
        shutil.rmtree(iconset)
    iconset.mkdir()

    iconset_sizes = [
        (16, "icon_16x16.png"),
        (32, "icon_16x16@2x.png"),
        (32, "icon_32x32.png"),
        (64, "icon_32x32@2x.png"),
        (128, "icon_128x128.png"),
        (256, "icon_128x128@2x.png"),
        (256, "icon_256x256.png"),
        (512, "icon_256x256@2x.png"),
        (512, "icon_512x512.png"),
        (1024, "icon_512x512@2x.png"),
    ]

    for size, filename in iconset_sizes:
        create_icon(size).save(iconset / filename, "PNG")

    subprocess.run(
        ["/usr/bin/iconutil", "-c", "icns", str(iconset), "-o", str(base / "icon.icns")],
        check=True,
    )
    print("Generated icon.icns")


def save_icons() -> None:
    base = Path(__file__).resolve().parent
    png_sizes = [
        (32, "32x32.png"),
        (128, "128x128.png"),
        (256, "128x128@2x.png"),
        (512, "icon_512x512.png"),
    ]

    for size, filename in png_sizes:
        create_icon(size).save(base / filename, "PNG")
        print(f"Generated {filename} ({size}x{size})")

    ico_sizes = [16, 32, 48, 64, 128, 256]
    ico_image = create_icon(256)
    ico_image.save(base / "icon.ico", "ICO", sizes=[(s, s) for s in ico_sizes])
    print("Generated icon.ico")

    build_icns(base)


if __name__ == "__main__":
    save_icons()
