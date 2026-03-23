import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw


BACKGROUND = (9, 9, 38, 255)
FOREGROUND = (255, 255, 255, 255)
RENDER_SCALE = 8


def create_icon(size: int) -> Image.Image:
    render_size = size * RENDER_SCALE
    img = Image.new("RGBA", (render_size, render_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    pad = max(1, round(render_size * 0.09))
    radius = round(render_size * 0.24)
    stroke = max(2, round(render_size * 0.055))

    draw.rounded_rectangle(
        [pad, pad, render_size - pad, render_size - pad],
        radius=radius,
        fill=BACKGROUND,
    )

    doc_left = round(render_size * 0.32)
    doc_top = round(render_size * 0.22)
    doc_right = round(render_size * 0.69)
    doc_bottom = round(render_size * 0.77)
    fold = round(render_size * 0.12)
    inner_radius = round(render_size * 0.045)

    draw.rounded_rectangle(
        [doc_left, doc_top, doc_right, doc_bottom],
        radius=inner_radius,
        outline=FOREGROUND,
        width=stroke,
    )

    # Remove the top-right corner so the fold looks cut out.
    draw.polygon(
        [
            (doc_right - fold, doc_top),
            (doc_right, doc_top),
            (doc_right, doc_top + fold),
        ],
        fill=BACKGROUND,
    )

    # Draw the folded corner.
    draw.line(
        [
            (doc_right - fold, doc_top),
            (doc_right - fold, doc_top + fold),
            (doc_right, doc_top + fold),
        ],
        fill=FOREGROUND,
        width=stroke,
        joint="curve",
    )

    code_y = round(render_size * 0.58)
    code_half_w = round(render_size * 0.072)
    code_h = round(render_size * 0.10)
    slash_h = round(render_size * 0.11)
    slash_w = max(2, round(render_size * 0.03))

    left_x = round(render_size * 0.42)
    right_x = round(render_size * 0.58)

    draw.line(
        [
            (left_x + code_half_w, code_y - code_h),
            (left_x - code_half_w, code_y),
            (left_x + code_half_w, code_y + code_h),
        ],
        fill=FOREGROUND,
        width=stroke,
        joint="curve",
    )

    draw.line(
        [
            (right_x - code_half_w, code_y - code_h),
            (right_x + code_half_w, code_y),
            (right_x - code_half_w, code_y + code_h),
        ],
        fill=FOREGROUND,
        width=stroke,
        joint="curve",
    )

    draw.line(
        [
            (round(render_size * 0.52), code_y - slash_h),
            (round(render_size * 0.48), code_y + slash_h),
        ],
        fill=FOREGROUND,
        width=slash_w,
    )

    return img.resize((size, size), Image.Resampling.LANCZOS)


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
