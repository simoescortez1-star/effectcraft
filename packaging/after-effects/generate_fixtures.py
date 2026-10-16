#!/usr/bin/env python3
"""Generate original tiny format regression data; no user files or Adobe assets."""
import argparse
from pathlib import Path
import struct


def chunk(tag, body):
    return tag + struct.pack(">I", len(body)) + body + (b"\0" if len(body) % 2 else b"")


def group(form, *children):
    return chunk(b"LIST", form + b"".join(children))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    options = parser.parse_args()
    options.out.mkdir(parents=True, exist_ok=True)
    descriptor = bytearray(124)
    descriptor[3] = 1
    layout = bytearray(52)
    struct.pack_into(">I", layout, 8, 2)
    struct.pack_into(">I", layout, 16, 48)
    keys = bytearray()
    for tick, value in [(0, 0.0), (8000, 1.0)]:
        key = bytearray(48)
        struct.pack_into(">I", key, 0, tick)
        key[4:6] = b"\2\2"
        struct.pack_into(">ddddd", key, 8, value, 0.0, 1.0/3, 0.0, 1.0/3)
        keys.extend(key)
    settings = group(b"sspc", group(b"tdbs", chunk(b"tdb4", descriptor), chunk(b"cdat", struct.pack(">d", 0.0)),
                                        group(b"list", chunk(b"lhd3", layout), chunk(b"ldat", keys))))
    path = group(b"tdsp", chunk(b"tdmn", b"ADBE Opacity"))
    sentinel = group(b"tdsp", chunk(b"tdmn", b"ADBE End of path sentinel"))
    preset = chunk(b"RIFX", b"FaFX" + group(b"besc", path, sentinel, settings))
    (options.out / "opacity.ffx").write_bytes(preset)
    # Unsupported effect with a named saved parameter, not a JSON masquerading as FFX.
    parameter = chunk(b"tdmn", b"org.effectcraft.test.unavailable-0001") + group(b"tdbs", chunk(b"tdb4", descriptor),chunk(b"cdat",struct.pack(">d",2.5)))
    unavailable = chunk(b"RIFX",b"FaFX"+group(b"besc",group(b"tdsp",chunk(b"tdmn",b"org.effectcraft.test.unavailable")),group(b"sspc",group(b"tdgp",parameter))))
    (options.out / "unavailable.ffx").write_bytes(unavailable)


if __name__ == "__main__":
    main()
