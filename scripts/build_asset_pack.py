#!/usr/bin/env python3
"""Build an asset pack for the pad's "assets" or "user" flash partition.

The layout is the one firmware/main/assets/asset_pack.h reads; the two change
together. Each asset is checked here, on the PC, so the pad only ever reads
what it can play:

  --gif NAME PATH            a GIF, stored as is
  --mjpeg NAME PATH FPS      a raw MJPEG stream (back-to-back JPEGs, as
                             `ffmpeg -f mjpeg` writes), split into frames;
                             baseline JPEG only (the pad's decoder, TJpgDec)

Assets are stored in the order given. Write the result into the build
directory as opad-assets.bin or opad-user.bin and `opadctl flash --full`
writes it with the firmware.

    python3 scripts/build_asset_pack.py --partition assets \\
        -o firmware/build/opad-assets.bin --label demo \\
        --mjpeg "updating media" updating.mjpeg 12
"""
import argparse
import csv
import struct
import sys
import zlib
from pathlib import Path

MAGIC = 0x4B41504F  # "OPAK"
FORMAT = 1
HEADER = struct.Struct("<IHHHHII16s28s")
ENTRY = struct.Struct("<32sB3sHHIIII8s")
KIND_GIF = 1
KIND_MJPEG = 2
assert HEADER.size == 64 and ENTRY.size == 64

REPO = Path(__file__).resolve().parent.parent
PARTITIONS = REPO / "firmware" / "partitions.csv"


def partition_size(name):
    with open(PARTITIONS) as f:
        rows = (r for r in csv.reader(l for l in f if not l.lstrip().startswith("#")))
        for row in rows:
            row = [c.strip() for c in row]
            if row and row[0] == name:
                return int(row[4], 0)
    sys.exit(f"no partition {name!r} in {PARTITIONS}")


def gif_info(data, path):
    if data[:6] not in (b"GIF87a", b"GIF89a"):
        sys.exit(f"{path}: not a GIF")
    width, height, flags = struct.unpack_from("<HHB", data, 6)
    pos = 13
    if flags & 0x80:
        pos += 3 << ((flags & 7) + 1)
    frames = 0

    def skip_sub_blocks(pos):
        while True:
            n = data[pos]
            pos += 1
            if n == 0:
                return pos
            pos += n

    while pos < len(data):
        b = data[pos]
        if b == 0x3B:  # trailer
            break
        if b == 0x21:  # extension
            pos = skip_sub_blocks(pos + 2)
        elif b == 0x2C:  # image
            frames += 1
            lflags = data[pos + 9]
            pos += 10
            if lflags & 0x80:
                pos += 3 << ((lflags & 7) + 1)
            pos = skip_sub_blocks(pos + 1)  # LZW minimum code size, then data
        else:
            sys.exit(f"{path}: malformed GIF at byte {pos}")
    return width, height, frames


def split_mjpeg(data, path):
    """Each JPEG in the stream, walked marker by marker (an FFD9 inside a
    segment or the entropy-coded data must not end a frame)."""
    frames = []
    pos = 0
    while pos < len(data):
        if data[pos:pos + 2] != b"\xff\xd8":
            if data[pos:].strip(b"\x00") == b"":
                break  # padding at the end
            sys.exit(f"{path}: no JPEG start at byte {pos}")
        start = pos
        pos += 2
        sof = None
        while True:
            if data[pos] != 0xFF:
                sys.exit(f"{path}: frame {len(frames)}: lost marker sync at byte {pos}")
            marker = data[pos + 1]
            if marker == 0xFF:  # fill byte
                pos += 1
                continue
            if marker == 0xD9:
                pos += 2
                break
            seg_len = struct.unpack_from(">H", data, pos + 2)[0]
            if marker in (0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF):
                sof = (marker, data[pos + 4:pos + 2 + seg_len])
            pos += 2 + seg_len
            if marker == 0xDA:  # entropy-coded data up to the next real marker
                while True:
                    if data[pos] == 0xFF and data[pos + 1] not in (0x00,) and not 0xD0 <= data[pos + 1] <= 0xD7:
                        break
                    pos += 1
        if sof is None:
            sys.exit(f"{path}: frame {len(frames)} has no SOF")
        frames.append((data[start:pos], sof))
    if not frames:
        sys.exit(f"{path}: no frames")

    marker, sof = frames[0][1]
    if marker != 0xC0:
        sys.exit(f"{path}: SOF{marker - 0xC0:X}; the pad decodes baseline JPEG (SOF0) only")
    height, width = struct.unpack_from(">HH", sof, 1)
    for i, (_, (m, s)) in enumerate(frames):
        if m != 0xC0 or struct.unpack_from(">HH", s, 1) != (height, width):
            sys.exit(f"{path}: frame {i} differs from frame 0 (type or size)")
    return width, height, [f for f, _ in frames]


class Assets(argparse.Action):
    def __call__(self, parser, namespace, values, option_string=None):
        items = getattr(namespace, "assets", None) or []
        items.append((option_string.lstrip("-"), values))
        namespace.assets = items


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-o", "--output", required=True, type=Path)
    ap.add_argument("--partition", required=True, choices=["assets", "user"],
                    help="the partition it is for; its size from firmware/partitions.csv is the limit")
    ap.add_argument("--label", default="", help="up to 16 bytes, shown in the pad's log")
    ap.add_argument("--gif", nargs=2, metavar=("NAME", "PATH"), action=Assets)
    ap.add_argument("--mjpeg", nargs=3, metavar=("NAME", "PATH", "FPS"), action=Assets)
    args = ap.parse_args()
    assets = getattr(args, "assets", None) or []
    if not assets:
        ap.error("no assets given")

    entries = []
    blobs = []
    for kind, values in assets:
        name, path = values[0], Path(values[1])
        if len(name.encode()) >= 32:
            sys.exit(f"name {name!r} is over 31 bytes")
        data = path.read_bytes()
        if kind == "gif":
            w, h, n = gif_info(data, path)
            entries.append((name, KIND_GIF, w, h, n, 0))
            blobs.append(data)
        else:
            fps = float(values[2])
            w, h, frames = split_mjpeg(data, path)
            ends = []
            end = 4 * len(frames)
            for f in frames:
                end += len(f)
                ends.append(end)
            blob = struct.pack(f"<{len(ends)}I", *ends) + b"".join(frames)
            entries.append((name, KIND_MJPEG, w, h, len(frames), round(1e6 / fps)))
            blobs.append(blob)
        e = entries[-1]
        print(f"  {e[0]:<32} {kind:<5} {e[2]}x{e[3]} {e[4]:4} frames {len(blobs[-1]) / 1024:8.1f} KB")

    pos = HEADER.size + ENTRY.size * len(entries)
    table = b""
    data = b""
    for (name, kind, w, h, n, frame_us), blob in zip(entries, blobs):
        pad = -pos % 4
        data += b"\x00" * pad
        pos += pad
        table += ENTRY.pack(name.encode(), kind, b"", w, h, n, frame_us, pos, len(blob), b"")
        data += blob
        pos += len(blob)
    body = table + data
    size = HEADER.size + len(body)
    limit = partition_size(args.partition)
    if size > limit:
        sys.exit(f"pack is {size} bytes; partition {args.partition!r} holds {limit}")
    header = HEADER.pack(MAGIC, FORMAT, HEADER.size, len(entries), ENTRY.size, size,
                         zlib.crc32(body), args.label.encode()[:16], b"")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(header + body)
    print(f"{args.output}: {size / 1024:.1f} KB of {limit / 1024:.0f} KB ({100 * size / limit:.0f}%)")


if __name__ == "__main__":
    main()
