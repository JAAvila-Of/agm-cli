#!/usr/bin/env python3
"""
Generate installer icon and branding assets from logo.png.

Single source of truth: repo-root `logo.png` (910x910 RGBA recommended).

Produces (all committed to the repo):
  crates/agm-cli/wix/agm.ico                       multi-size .ico (16..256)
  crates/agm-cli/wix/WixUIBanner.bmp               493x58  WiX top banner
  crates/agm-cli/wix/WixUIDialog.bmp               493x312 WiX welcome dialog
  crates/agm-cli/installer/macos/agm.icns          full multi-size .icns
  crates/agm-cli/installer/macos/Resources/background.png  620x418 pkg BG

Requires: Pillow (`pip install pillow`).

Run from repo root:
    python scripts/gen-icons.py
"""

from __future__ import annotations

import io
import struct
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "logo.png"
WIX_DIR = ROOT / "crates" / "agm-cli" / "wix"
MAC_DIR = ROOT / "crates" / "agm-cli" / "installer" / "macos"
MAC_RES = MAC_DIR / "Resources"

ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

# ICNS entries: (4-byte OSType, pixel size). All types below accept PNG data.
# Mapping follows Apple's iconset -> icns conventions.
ICNS_ENTRIES: list[tuple[bytes, int]] = [
    (b"icp4", 16),    # 16x16
    (b"icp5", 32),    # 32x32
    (b"ic07", 128),   # 128x128
    (b"ic08", 256),   # 256x256
    (b"ic09", 512),   # 512x512
    (b"ic10", 1024),  # 1024x1024 / 512@2x
    (b"ic11", 32),    # 16x16@2x
    (b"ic12", 64),    # 32x32@2x
    (b"ic13", 256),   # 128x128@2x
    (b"ic14", 512),   # 256x256@2x
]


def load_logo() -> Image.Image:
    if not SRC.exists():
        print(f"error: {SRC} not found", file=sys.stderr)
        sys.exit(1)
    return Image.open(SRC).convert("RGBA")


def _paste_centered(canvas: Image.Image, logo: Image.Image, max_dim: int, y_offset: int = 0) -> None:
    scale = max_dim / max(logo.size)
    lw = int(logo.size[0] * scale)
    lh = int(logo.size[1] * scale)
    lg = logo.resize((lw, lh), Image.LANCZOS)
    x = (canvas.width - lw) // 2
    y = (canvas.height - lh) // 2 + y_offset
    mask = lg.split()[3] if lg.mode == "RGBA" else None
    canvas.paste(lg, (x, y), mask)


def gen_ico(logo: Image.Image) -> None:
    WIX_DIR.mkdir(parents=True, exist_ok=True)
    out = WIX_DIR / "agm.ico"
    logo.save(out, format="ICO", sizes=[(s, s) for s in ICO_SIZES])
    print(f"wrote {out.relative_to(ROOT)} ({out.stat().st_size} bytes)")


def gen_banner(logo: Image.Image) -> None:
    # WixUIBannerBmp: 493x58, top-right area of most dialogs.
    w, h = 493, 58
    canvas = Image.new("RGB", (w, h), "white")
    lg_size = h - 8
    lg = logo.resize((lg_size, lg_size), Image.LANCZOS)
    mask = lg.split()[3] if lg.mode == "RGBA" else None
    # Place logo on the right side (WiX convention).
    canvas.paste(lg, (w - lg_size - 10, 4), mask)
    out = WIX_DIR / "WixUIBanner.bmp"
    canvas.save(out, format="BMP")
    print(f"wrote {out.relative_to(ROOT)}")


def gen_dialog(logo: Image.Image) -> None:
    # WixUIDialogBmp: 493x312, welcome + completion dialogs.
    w, h = 493, 312
    canvas = Image.new("RGB", (w, h), "white")
    _paste_centered(canvas, logo, max_dim=200, y_offset=-10)
    out = WIX_DIR / "WixUIDialog.bmp"
    canvas.save(out, format="BMP")
    print(f"wrote {out.relative_to(ROOT)}")


def gen_icns(logo: Image.Image) -> None:
    MAC_DIR.mkdir(parents=True, exist_ok=True)
    blocks: list[bytes] = []
    for code, size in ICNS_ENTRIES:
        img = logo.resize((size, size), Image.LANCZOS)
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        data = buf.getvalue()
        length = 8 + len(data)
        blocks.append(code + struct.pack(">I", length) + data)
    body = b"".join(blocks)
    header = b"icns" + struct.pack(">I", 8 + len(body))
    out = MAC_DIR / "agm.icns"
    out.write_bytes(header + body)
    print(f"wrote {out.relative_to(ROOT)} ({out.stat().st_size} bytes)")


def gen_background(logo: Image.Image) -> None:
    # macOS installer background, classic size.
    w, h = 620, 418
    canvas = Image.new("RGBA", (w, h), (255, 255, 255, 255))
    _paste_centered(canvas, logo, max_dim=260, y_offset=-20)
    MAC_RES.mkdir(parents=True, exist_ok=True)
    out = MAC_RES / "background.png"
    canvas.convert("RGB").save(out, format="PNG")
    print(f"wrote {out.relative_to(ROOT)}")


def main() -> None:
    logo = load_logo()
    gen_ico(logo)
    gen_banner(logo)
    gen_dialog(logo)
    gen_icns(logo)
    gen_background(logo)
    print("done.")


if __name__ == "__main__":
    main()
