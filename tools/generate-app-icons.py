#!/usr/bin/env python3
"""Generates every platform's app icon from the shared logo artwork.

Source: apps/macos/generated_images/rays-fade-clockwise-v1.png — the supplied
render of the Moonglow mark (a gradient crescent inside a black plate, with
eight rays). The render has a white page background, its plate is a little
wider than tall, and the plate's top and bottom edges sit on the canvas edge.

Outputs:
    apps/macos/Resources/Moonglow.png                 1024 px master
    apps/macos/Resources/Moonglow-small.png           crescent mark, 256 px
    apps/windows/Assets/Moonglow.png                  full logo, 512 px
    apps/windows/Assets/Moonglow-small.png            crescent mark, 256 px
    apps/windows/Assets/Moonglow.ico                  mixed-size Windows icon
    apps/linux/packaging/icons/hicolor/*/apps/moonglow.png

The white page is knocked out and the plate is restored to a square with a 2%
transparent margin, matching the previous icon's breathing room. Frames at or
below 32 px use a crescent-only variant: at that size the rays fall below a
pixel and turn into noise, so the moon is enlarged to stay recognisable.

Usage: python3 tools/generate-app-icons.py [source.png]
Requires Pillow.
"""

from __future__ import annotations

import io
import struct
import sys
from collections import deque
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SOURCE = Path(
    sys.argv[1]
    if len(sys.argv) > 1
    else ROOT / "apps/macos/generated_images/rays-fade-clockwise-v1.png"
)

MACOS_ICON = ROOT / "apps/macos/Resources/Moonglow.png"
MACOS_SMALL = ROOT / "apps/macos/Resources/Moonglow-small.png"
WINDOWS_ICON = ROOT / "apps/windows/Assets/Moonglow.png"
WINDOWS_SMALL = ROOT / "apps/windows/Assets/Moonglow-small.png"
WINDOWS_ICO = ROOT / "apps/windows/Assets/Moonglow.ico"
LINUX_ICONS = ROOT / "apps/linux/packaging/icons/hicolor"

PLATE = (6, 5, 6)
MARGIN = 0.022
CORNER = 0.259  # corner radius as a fraction of the plate; matches the render
SMALL_MAX = 32
LINUX_SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)


def luminance_range(image: Image.Image) -> Image.Image:
    r, g, b = image.convert("RGB").split()
    from PIL import ImageChops

    return ImageChops.darker(ImageChops.darker(r, g), b)


def plate_bounds(image: Image.Image) -> tuple[int, int, int, int]:
    """Bounds of the dark plate, ignoring the white page."""
    mask = luminance_range(image).point(lambda value: 255 if value < 128 else 0)
    bounds = mask.getbbox()
    if bounds is None:
        raise SystemExit(f"no plate found in {SOURCE}")
    return bounds


def knock_out_page(image: Image.Image) -> Image.Image:
    """Makes the white page transparent, keeping white artwork inside the plate."""
    rgba = image.convert("RGBA")
    if rgba.getchannel("A").getextrema()[0] < 128:
        return rgba  # already transparent

    work = image.convert("RGB").copy()
    ImageDraw.floodfill(work, (0, 0), (255, 0, 255), thresh=200)
    alpha = Image.new("L", work.size, 255)
    wp, ap = work.load(), alpha.load()
    for y in range(work.height):
        for x in range(work.width):
            if wp[x, y] == (255, 0, 255):
                ap[x, y] = 0

    rgba.putalpha(alpha)
    return rgba


def square_plate(source: Image.Image) -> Image.Image:
    """Crops to the plate and restores the square the render was cropped from."""
    left, top, right, bottom = plate_bounds(source)
    plate = source.crop((left, top, right, bottom))
    side = max(plate.size)
    return plate.resize((side, side), Image.LANCZOS)


def with_margin(plate: Image.Image, size: int) -> Image.Image:
    pad = round(plate.width * MARGIN)
    canvas = Image.new("RGBA", (plate.width + 2 * pad, plate.height + 2 * pad), (0, 0, 0, 0))
    canvas.paste(plate, (pad, pad), plate)
    return canvas.resize((size, size), Image.LANCZOS)


def crescent_only(plate: Image.Image) -> Image.Image:
    """The moon as its own plate, enlarged; drops the rays that vanish at 16 px."""
    px = plate.load()
    side = plate.width
    saturated = [
        (x, y)
        for y in range(0, side, 2)
        for x in range(0, side, 2)
        if max(px[x, y][:3]) - min(px[x, y][:3]) > 40 and max(px[x, y][:3]) > 60
    ]
    if not saturated:
        return plate

    middle = (min(y for _, y in saturated) + max(y for _, y in saturated)) // 2
    seed = next(
        (x, middle)
        for x in range(side)
        if max(px[x, middle][:3]) > 22 and max(px[x, middle][:3]) - min(px[x, middle][:3]) > 24
    )

    def bright(x: int, y: int) -> bool:
        return px[x, y][3] > 128 and max(px[x, y][:3]) > 22

    seen = {seed}
    queue = deque([seed])
    while queue:
        x, y = queue.popleft()
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)):
            nx, ny = x + dx, y + dy
            if 0 <= nx < side and 0 <= ny < side and (nx, ny) not in seen and bright(nx, ny):
                seen.add((nx, ny))
                queue.append((nx, ny))

    xs = [x for x, _ in seen]
    ys = [y for _, y in seen]
    width, height = max(xs) - min(xs), max(ys) - min(ys)
    box = round(max(width, height) * 1.35)
    cx, cy = (min(xs) + max(xs)) // 2, (min(ys) + max(ys)) // 2

    backdrop = Image.new("RGBA", (box, box), PLATE + (255,))
    moon = plate.copy()
    moon.putalpha(
        Image.composite(
            moon.getchannel("A"),
            Image.new("L", moon.size, 0),
            Image.frombytes("L", moon.size, bytes(
                255 if (x, y) in seen else 0
                for y in range(side)
                for x in range(side)
            )),
        )
    )
    backdrop.paste(moon, (box // 2 - cx, box // 2 - cy), moon)

    rounded = Image.new("L", (box, box), 0)
    ImageDraw.Draw(rounded).rounded_rectangle(
        (0, 0, box - 1, box - 1), radius=round(box * CORNER), fill=255
    )
    backdrop.putalpha(rounded)
    return backdrop


def dib_frame(frame: Image.Image) -> bytes:
    """Classic 32-bit BMP frame plus AND mask: the form every icon reader accepts."""
    width, height = frame.size
    rgba = frame.convert("RGBA")
    pixels = bytearray()
    mask_stride = ((width + 31) // 32) * 4
    mask = bytearray()
    for y in range(height - 1, -1, -1):
        mask_row = bytearray(mask_stride)
        for x in range(width):
            r, g, b, a = rgba.getpixel((x, y))
            pixels += bytes((b, g, r, a))
            if a == 0:
                mask_row[x // 8] |= 0x80 >> (x % 8)
        mask += mask_row
    header = struct.pack(
        "<IiiHHIIiiII", 40, width, height * 2, 1, 32, 0, len(pixels) + len(mask), 0, 0, 0, 0
    )
    return header + bytes(pixels) + bytes(mask)


def write_ico(path: Path, frames: list[tuple[int, Image.Image]]) -> None:
    """Small frames use BMP data for compatibility; large ones use PNG to stay compact."""
    payloads = []
    for size, frame in frames:
        if size < 128:
            payloads.append((size, dib_frame(frame)))
        else:
            buffer = io.BytesIO()
            frame.save(buffer, format="PNG")
            payloads.append((size, buffer.getvalue()))

    header = struct.pack("<HHH", 0, 1, len(payloads))
    entries = b""
    offset = 6 + 16 * len(payloads)
    for size, payload in payloads:
        dimension = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dimension, dimension, 0, 0, 1, 32, len(payload), offset)
        offset += len(payload)

    path.write_bytes(header + entries + b"".join(payload for _, payload in payloads))


def main() -> None:
    source = Image.open(SOURCE)
    plate = square_plate(knock_out_page(source))
    small = crescent_only(plate)

    full_master = with_margin(plate, 1024)
    small_master = with_margin(small, 1024)

    MACOS_ICON.parent.mkdir(parents=True, exist_ok=True)
    full_master.save(MACOS_ICON, optimize=True)
    with_margin(small, 256).save(MACOS_SMALL, optimize=True)

    WINDOWS_ICON.parent.mkdir(parents=True, exist_ok=True)
    with_margin(plate, 512).save(WINDOWS_ICON, optimize=True)
    with_margin(small, 256).save(WINDOWS_SMALL, optimize=True)

    frames = []
    for size in ICO_SIZES:
        master = small_master if size <= SMALL_MAX else full_master
        frames.append((size, master.resize((size, size), Image.LANCZOS)))
    write_ico(WINDOWS_ICO, frames)

    for size in LINUX_SIZES:
        master = small_master if size <= SMALL_MAX else full_master
        target = LINUX_ICONS / f"{size}x{size}/apps/moonglow.png"
        target.parent.mkdir(parents=True, exist_ok=True)
        master.resize((size, size), Image.LANCZOS).save(target, optimize=True)

    print(f"source:  {SOURCE.relative_to(ROOT)}")
    for path in (MACOS_ICON, MACOS_SMALL, WINDOWS_ICON, WINDOWS_SMALL, WINDOWS_ICO):
        print(f"wrote:   {path.relative_to(ROOT)}")
    print(f"wrote:   {LINUX_ICONS.relative_to(ROOT)}/{{{'|'.join(str(s) for s in LINUX_SIZES)}}}x/apps/moonglow.png")


if __name__ == "__main__":
    main()
