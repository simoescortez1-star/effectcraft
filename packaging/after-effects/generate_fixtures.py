#!/usr/bin/env python3
"""Generate original tiny format regression data; no user files or Adobe assets."""
import argparse
from pathlib import Path
import struct


def chunk(tag, body):
    return tag + struct.pack(">I", len(body)) + body + (b"\0" if len(body) % 2 else b"")


def group(form, *children):
    return chunk(b"LIST", form + b"".join(children))


def saved_clock(rate):
    data = bytearray(56)
    struct.pack_into(">IIII", data, 0, 1, 1, rate * 5, rate)
    struct.pack_into(">I", data, 16, 60 << 16)
    return chunk(b"beso", data)


def target_path(name):
    return group(b"tdsp", *(group(b"tdsi", chunk(b"tdmn", part.encode() + b"\0"))
                            for part in ["ADBE Transform Group", name]))


def numeric_settings(dimensions, rate, keys, *, bezier=True):
    descriptor = bytearray(124)
    descriptor[3] = dimensions
    struct.pack_into(">I", descriptor, 12, rate)
    layout = bytearray(52)
    stride = 8 + 5 * dimensions * 8
    struct.pack_into(">I", layout, 8, len(keys))
    struct.pack_into(">I", layout, 16, stride)
    data = bytearray()
    for tick, values in keys:
        key = bytearray(stride)
        struct.pack_into(">I", key, 0, tick)
        key[4:6] = b"\2\2" if bezier else b"\1\1"
        fields = list(values) + [0.0] * dimensions + [1 / 3] * dimensions + [0.0] * dimensions + [1 / 3] * dimensions
        struct.pack_into(">" + "d" * len(fields), key, 8, *fields)
        data.extend(key)
    return group(b"tdbs", chunk(b"tdb4", descriptor), group(b"list", chunk(b"lhd3", layout), chunk(b"ldat", data)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    options = parser.parse_args()
    options.out.mkdir(parents=True, exist_ok=True)
    descriptor = bytearray(124)
    descriptor[3] = 1
    struct.pack_into(">I", descriptor, 12, 8000)
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
    preset = chunk(b"RIFX", b"FaFX" + group(b"besc", saved_clock(8000), path, sentinel, settings))
    (options.out / "opacity.ffx").write_bytes(preset)
    # Direct tdbs storage, nested stable match-name path and nonzero key offsets.
    # Values/times are original test data, not the private uploaded Scale preset.
    scale = numeric_settings(3, 48000, [(24000, [0.5, 0.75, 1.0]), (48000, [1.5, 1.25, 1.0])])
    body = saved_clock(48000) + target_path("ADBE Scale") + chunk(b"tdsn", b"Different display label") + sentinel + scale
    (options.out / "scale.ffx").write_bytes(chunk(b"RIFX", b"FaFX" + group(b"besc", body)))
    # Conflicting container declarations are invalid; per-property clocks may differ.
    wrong_clock = saved_clock(24000) + saved_clock(48000) + target_path("ADBE Scale") + sentinel + scale
    (options.out / "conflicting-clock.ffx").write_bytes(chunk(b"RIFX", b"FaFX" + group(b"besc", wrong_clock)))
    records = []
    for name, values in [("ADBE Position", [16., 16.]), ("ADBE Anchor Point", [16., 16.]),
                         ("ADBE Rotate Z", [20.]), ("ADBE Opacity", [0.6])]:
        records.extend([target_path(name), sentinel, numeric_settings(len(values), 48000, [(0, values), (24000, values)], bezier=False)])
    (options.out / "transforms.ffx").write_bytes(chunk(b"RIFX", b"FaFX" + group(b"besc", saved_clock(48000), *records)))
    # Unsupported effect with a named saved parameter, not a JSON masquerading as FFX.
    parameter = chunk(b"tdmn", b"org.effectcraft.test.unavailable-0001") + group(b"tdbs", chunk(b"tdb4", descriptor),chunk(b"cdat",struct.pack(">d",2.5)))
    unavailable = chunk(b"RIFX",b"FaFX"+group(b"besc",group(b"tdsp",chunk(b"tdmn",b"org.effectcraft.test.unavailable")),group(b"sspc",group(b"tdgp",parameter))))
    (options.out / "unavailable.ffx").write_bytes(unavailable)


if __name__ == "__main__":
    main()
