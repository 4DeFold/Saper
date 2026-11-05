#!/usr/bin/env python3
"""Генерация собственной иконки приложения (только стандартная библиотека).

Рисует классическую серую выпуклую плитку с миной. Никаких чужих изображений.
Результат: assets/icon.png, 1024x1024 RGBA.
"""
from __future__ import annotations

import math
import os
import struct
import zlib

W = H = 1024
FACE = (200, 200, 200, 255)
LIGHT = (248, 248, 248, 255)
SHADE = (96, 96, 96, 255)
INSET = (170, 170, 170, 255)
BLACK = (26, 26, 26, 255)
WHITE = (250, 250, 250, 255)
RED = (205, 30, 30, 255)

buf = bytearray(W * H * 4)


def rect(x0: int, y0: int, x1: int, y1: int, color: tuple[int, int, int, int]) -> None:
    x0, y0 = max(0, x0), max(0, y0)
    x1, y1 = min(W, x1), min(H, y1)
    if x1 <= x0 or y1 <= y0:
        return
    row = bytes(color) * (x1 - x0)
    for y in range(y0, y1):
        off = (y * W + x0) * 4
        buf[off : off + len(row)] = row


def circle(cx: float, cy: float, r: float, color: tuple[int, int, int, int]) -> None:
    x0, y0 = int(cx - r - 1), int(cy - r - 1)
    x1, y1 = int(cx + r + 2), int(cy + r + 2)
    r2 = r * r
    for y in range(max(0, y0), min(H, y1)):
        for x in range(max(0, x0), min(W, x1)):
            if (x - cx) ** 2 + (y - cy) ** 2 <= r2:
                off = (y * W + x) * 4
                buf[off : off + 4] = bytes(color)


def thick_line(
    x0: float, y0: float, x1: float, y1: float, radius: float, color: tuple[int, int, int, int]
) -> None:
    x0b, y0b = int(min(x0, x1) - radius - 1), int(min(y0, y1) - radius - 1)
    x1b, y1b = int(max(x0, x1) + radius + 2), int(max(y0, y1) + radius + 2)
    dx, dy = x1 - x0, y1 - y0
    length2 = dx * dx + dy * dy or 1.0
    r2 = radius * radius
    for y in range(max(0, y0b), min(H, y1b)):
        for x in range(max(0, x0b), min(W, x1b)):
            t = ((x - x0) * dx + (y - y0) * dy) / length2
            t = min(1.0, max(0.0, t))
            px, py = x0 + t * dx, y0 + t * dy
            if (x - px) ** 2 + (y - py) ** 2 <= r2:
                off = (y * W + x) * 4
                buf[off : off + 4] = bytes(color)


def triangle(
    a: tuple[float, float],
    b: tuple[float, float],
    c: tuple[float, float],
    color: tuple[int, int, int, int],
) -> None:
    x0 = int(min(a[0], b[0], c[0]))
    x1 = int(max(a[0], b[0], c[0])) + 1
    y0 = int(min(a[1], b[1], c[1]))
    y1 = int(max(a[1], b[1], c[1])) + 1

    def sign(p, q, r):
        return (p[0] - r[0]) * (q[1] - r[1]) - (q[0] - r[0]) * (p[1] - r[1])

    for y in range(max(0, y0), min(H, y1)):
        for x in range(max(0, x0), min(W, x1)):
            p = (x + 0.5, y + 0.5)
            d1, d2, d3 = sign(p, a, b), sign(p, b, c), sign(p, c, a)
            if (d1 >= 0 and d2 >= 0 and d3 >= 0) or (d1 <= 0 and d2 <= 0 and d3 <= 0):
                off = (y * W + x) * 4
                buf[off : off + 4] = bytes(color)


def chunk(tag: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def main() -> None:
    # Выпуклая плитка.
    rect(32, 32, 992, 992, FACE)
    rect(32, 32, 992, 80, LIGHT)
    rect(32, 32, 80, 992, LIGHT)
    rect(32, 944, 992, 992, SHADE)
    rect(944, 32, 992, 992, SHADE)
    rect(96, 96, 928, 928, INSET)

    # Мина: шипы, тело, блик.
    cx, cy, r = 512.0, 500.0, 150.0
    for i in range(8):
        angle = i * math.pi / 4.0
        dx, dy = math.cos(angle), math.sin(angle)
        thick_line(cx + dx * r * 0.7, cy + dy * r * 0.7, cx + dx * r * 1.75, cy + dy * r * 1.75, 22, BLACK)
    circle(cx, cy, r, BLACK)
    circle(cx - r * 0.30, cy - r * 0.32, r * 0.20, WHITE)

    # Маленький красный флажок в углу — собственный декоративный элемент.
    thick_line(735, 690, 735, 880, 16, BLACK)
    thick_line(690, 880, 800, 880, 16, BLACK)
    triangle((738, 692), (738, 790), (880, 735), RED)

    raw = bytearray()
    for y in range(H):
        raw.append(0)  # PNG filter: None
        raw.extend(buf[y * W * 4 : (y + 1) * W * 4])
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )
    out = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets", "icon.png")
    with open(out, "wb") as handle:
        handle.write(png)
    print(f"записано: {out} ({len(png)} байт)")


if __name__ == "__main__":
    main()
