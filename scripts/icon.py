# Generates assets/icon-1024.png and icon-256.png: an island in the sea (stdlib only).
import math, struct, zlib, sys

def render(n):
    px = bytearray()
    for y in range(n):
        px.append(0)
        for x in range(n):
            u, v = x / n, y / n
            r = 0.47 * n
            dx, dy = x - n / 2, y - n / 2
            corner = max(abs(dx), abs(dy)) > r and math.hypot(max(abs(dx) - r * 0.78, 0), max(abs(dy) - r * 0.78, 0)) > r * 0.22
            if corner:
                px += b"\0\0\0\0"; continue
            sky = (int(120 + 100 * v), int(190 + 50 * v), 235)
            c = sky
            if v > 0.62:
                c = (int(16 + 20 * (v - 0.62)), int(110 - 40 * (v - 0.62)), int(190 - 60 * (v - 0.62)))
                if math.sin(u * 40 + v * 9) > 0.93: c = (150, 210, 235)
            sun = math.hypot(u - 0.74, v - 0.26)
            if sun < 0.09: c = (255, 214, 90)
            island = ((u - 0.45) / 0.3) ** 2 + ((v - 0.66) / 0.07) ** 2
            if island < 1 and v < 0.69: c = (236, 205, 130)
            trunk = abs(u - (0.47 + (0.66 - v) * 0.25)) < 0.018 and 0.36 < v < 0.64
            if trunk: c = (120, 78, 40)
            for ang in (-2.6, -2.0, -1.2, -0.5, 0.2):
                tx, ty = 0.555, 0.36
                t = ((u - tx) * math.cos(ang) + (v - ty) * math.sin(ang))
                s = (-(u - tx) * math.sin(ang) + (v - ty) * math.cos(ang))
                if 0 < t < 0.2 and abs(s - 0.25 * t * t / 0.2) < 0.035 * (1 - t / 0.2) + 0.006:
                    c = (40, 140, 60)
            px += bytes(c) + b"\xff"
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", n, n, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(bytes(px), 9)) + chunk(b"IEND", b"")

for n in (1024, 256):
    open(f"assets/icon-{n}.png", "wb").write(render(n))
