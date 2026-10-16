#!/usr/bin/env python3
"""Require actual Windows editor/bridge render tests before packaging this edition."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--converter-work", type=Path, required=True)
    parser.add_argument("--native-work", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    options = parser.parse_args()
    if os.name != "nt":
        parser.error("these are native Windows tests")
    root = Path(__file__).resolve().parents[2]
    out = options.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    subprocess.run(["python", str(Path(__file__).with_name("generate_fixtures.py")), "--out", str(out)], check=True)
    native = options.native_work.resolve() / "target/x86_64-pc-windows-msvc/release"
    plugin = out / "EffectCraft Gain.aex"
    shutil.copyfile(native / "effectcraft_aex_gain_fixture.dll", plugin)
    env = os.environ.copy()
    env.update(EFFECTCRAFT_AEP_CONVERTER=str(options.converter_work.resolve() / "target/x86_64-pc-windows-msvc/release/aep-effectcraft.exe"),
               EFFECTCRAFT_AEX_WORKER=str(native / "effectcraft-aex-worker.exe"),
               EFFECTCRAFT_TEST_FFX=str(out / "opacity.ffx"), EFFECTCRAFT_TEST_AEX=str(plugin))
    results = {}
    for test in ["ffx_external_generated_end_to_end", "ffx_direct_transform_presets_render_and_preserve_saved_timebase",
                 "aex_native_worker_renders_and_recovers_after_child_exit"]:
        result = subprocess.run(["cargo", "+1.95.0", "test", "--locked", "--release", "--target", "x86_64-pc-windows-msvc",
                                 "-p", "effectcraft-engine", "--lib", f"tests_after_effects::{test}", "--", "--ignored", "--exact", "--nocapture"],
                                cwd=root, env=env, text=True, encoding="utf-8", capture_output=True, errors="replace", timeout=900)
        log = result.stdout + result.stderr
        (out / f"{test}.txt").write_text(log, encoding="utf-8")
        print(log, flush=True)
        if result.returncode != 0 or "1 passed; 0 failed" not in log:
            raise RuntimeError(f"required native Windows render regression failed or did not run: {test}")
        results[test] = True
    results["source_commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    results["target"] = "x86_64-pc-windows-msvc"
    (out / "WINDOWS-VALIDATION.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
