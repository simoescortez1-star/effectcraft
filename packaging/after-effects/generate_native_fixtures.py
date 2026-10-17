"""Original FFX protocol regressions, not After Effects-authored visual references."""
import json
import struct
from pathlib import Path
from generate_fixtures import chunk, group, saved_clock, numeric_settings


def value(values, rate=24000):
    descriptor = bytearray(124)
    descriptor[3] = len(values)
    struct.pack_into(">I", descriptor, 12, rate)
    return group(b"tdbs", chunk(b"tdb4", descriptor), chunk(b"cdat", struct.pack(">" + "d" * len(values), *values)))


def animated(values, *, bezier=True, offset=0, rate=24000, interpolation=None):
    return numeric_settings(len(values[0]), rate, [(offset, values[0]), (offset + rate, values[1])], bezier=bezier, interpolation=interpolation)


def effect(match_name, parameters):
    children = []
    for suffix, settings in parameters:
        children.extend([chunk(b"tdmn", f"{match_name}-{suffix}".encode()), settings])
    return (match_name, group(b"sspc", group(b"tdgp", *children)))


def warp():
    return effect("ADBE WRPMESH", [("0001", value([4.])), ("0002", value([1.])),
                  ("0003", animated([[30.], [0.]])), ("0004", value([0.])), ("0005", value([0.]))])


def tile():
    return effect("ADBE Tile", [("0001", value([.5, .5])), ("0002", value([50.])),
                  ("0003", value([50.])), ("0004", value([400.])), ("0005", value([400.])),
                  ("0006", value([1.])), ("0007", value([0.])), ("0008", value([0.]))])


def tint():
    return effect("ADBE Tint", [("0001", value([255., 0., 20., 0.])),
                  ("0002", value([255., 250., 220., 30.])), ("0003", animated([[80.], [10.]]))])


def gaussian():
    return effect("ADBE Gaussian Blur 2", [("0001", animated([[2.], [5.]])),
                  ("0002", value([1.])), ("0003", value([1.]))])


def declared_effect(match_name, records):
    """Self-described original controls; arbitrary test IDs are not AE reference IDs."""
    table = [chunk(b"tdmn", (match_name + "-0000").encode()), chunk(b"pard", bytes(148))]
    values = []
    for suffix, label, control, settings in records:
        name = f"{match_name}-{suffix}"
        definition = bytearray(148)
        definition[15] = control
        text = label.encode("ascii")
        if len(text) > 31:
            raise ValueError("test declaration label too long")
        definition[16:16+len(text)] = text
        table.extend([chunk(b"tdmn", name.encode()), chunk(b"pard", definition)])
        values.extend([chunk(b"tdmn", name.encode()), settings])
    return (match_name, group(b"sspc", group(b"parT", *table), group(b"tdgp", *values)))


def generate(out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    position = ("ADBE Position", animated([[28., 32., 0.], [38., 30., 0.]]))
    scale = ("ADBE Scale", animated([[.7, .8, 1.], [1.2, 1.1, 1.]]))
    rotation = ("ADBE Rotate Z", animated([[20.], [-35.]], bezier=False))
    anchor = ("ADBE Anchor Point", animated([[4., 4., 0.], [10., 8., 0.]]))
    opacity = ("ADBE Opacity", animated([[.3], [.8]]))
    glow = declared_effect("ADBE Glo2", [("0201", "Glow Threshold", 10, value([10.])),
        ("0202", "Glow Radius", 10, value([4.])), ("0203", "Glow Intensity", 10, animated([[2.], [.5]]))])
    transform = declared_effect("ADBE Geometry2", [("0401", "Rotation", 3, animated([[20.], [0.]])),
        ("0402", "Skew", 10, value([10.])), ("0403", "Opacity", 10, value([75.]))])
    cases = [
        ("native-scale", [scale], 1, 0, 2),
        ("native-position", [position], 1, 0, 2),
        ("native-rotation", [rotation], 1, 0, 2),
        ("native-anchor", [anchor], 1, 0, 2),
        ("native-opacity", [opacity], 1, 0, 2),
        ("native-transform-combined", [position, scale, rotation], 3, 0, 6),
        ("native-bezier", [("ADBE Scale", animated([[.6, .6, 1.], [1.4, 1.4, 1.]]))], 1, 0, 2),
        ("native-hold", [("ADBE Opacity", animated([[.3], [.8]], interpolation=3))], 1, 0, 2),
        ("native-warp", [warp()], 0, 1, 2),
        ("native-scale-motion-tile", [scale, tile()], 1, 1, 2),
        ("native-gaussian", [gaussian()], 0, 1, 2),
        ("native-glow-declared", [glow], 0, 1, 2),
        ("native-transform-declared", [transform], 0, 1, 2),
        ("native-two-effects", [tint(), warp()], 0, 2, 4),
        ("native-partial", [scale, effect("org.cortezeffects.test.unavailable", [("0001", value([2.]))]), warp()], 1, 1, 4),
        ("native-multiple-animated", [position, scale, opacity, warp()], 3, 1, 8),
        ("native-rotation-anchor", [rotation, anchor], 2, 0, 4),
        ("native-opacity-transform", [opacity, transform], 1, 1, 4),
        ("native-warp-scale", [warp(), scale], 1, 1, 4),
        ("native-skew", [("ADBE Skew", animated([[25.], [-10.]])), ("ADBE Skew Axis", value([15.]))], 2, 0, 2),
        ("native-separated-position", [("ADBE Position_0", animated([[24.], [40.]])), ("ADBE Position_1", animated([[28.], [36.]], offset=6000))], 2, 0, 4),
        ("native-orientation", [("ADBE Orientation", animated([[20., 10., 0.], [0., 0., 0.]]))], 1, 0, 2),
        ("native-x-rotation", [("ADBE Rotate X", animated([[30.], [0.]]))], 1, 0, 2),
        ("native-y-rotation", [("ADBE Rotate Y", animated([[25.], [0.]]))], 1, 0, 2),
        ("native-z-position", [("ADBE Position_2", animated([[20.], [0.]]))], 1, 0, 2),
    ]
    manifest = []
    for name, records, properties, effects, keys in cases:
        paths, settings = [], []
        for match_name, payload in records:
            ancestor = "ADBE Effect Parade" if payload[8:12] == b"sspc" else "ADBE Transform Group"
            paths.append(group(b"tdsp", *(group(b"tdsi", chunk(b"tdmn", item.encode())) for item in [ancestor, match_name])))
            settings.append(payload)
        sentinel = group(b"tdsp", chunk(b"tdmn", b"ADBE End of path sentinel"))
        # Deliberately different container/property clocks and batched target paths.
        data = chunk(b"RIFX", b"FaFX" + group(b"besc", saved_clock(30000), *paths, sentinel, *settings))
        (out / (name + ".ffx")).write_bytes(data)
        manifest.append({"file": name + ".ffx", "properties": properties, "effects": effects, "keys": keys,
                         "partial": name == "native-partial", "threeD": name in {"native-orientation", "native-x-rotation", "native-y-rotation", "native-z-position"}, "authoredByAfterEffects": False})
    (out / "native-cases.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    generate(parser.parse_args().out)
