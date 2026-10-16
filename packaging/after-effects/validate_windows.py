#!/usr/bin/env python3
"""Require actual Windows editor/bridge render tests before packaging this edition."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import struct


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
    # Run the exact portable CLI arrangement users receive; no Python on their PC.
    diagnostic = root / "target/x86_64-pc-windows-msvc/release/effectcraft-aex-diagnostic.exe"
    shutil.copyfile(diagnostic, out / diagnostic.name)
    shutil.copyfile(native / "effectcraft-aex-worker.exe", out / "effectcraft-aex-worker.exe")
    shutil.copyfile(env["EFFECTCRAFT_AEP_CONVERTER"], out / "aep-effectcraft.exe")
    diagnostic_env = env.copy()
    diagnostic_env.pop("EFFECTCRAFT_AEX_WORKER", None)
    diagnostic_env.pop("EFFECTCRAFT_AEP_CONVERTER", None)
    test_plugins = out / "test-plugins"
    test_plugins.mkdir()
    shutil.copyfile(plugin, test_plugins / "EffectCraft Gain.aex")
    passive = out / "diagnostic-passive.json"
    local = out / "aex-local-diagnostic.json"
    log = []
    for args in [["--folder", str(test_plugins), "--out", str(passive)],
                 ["--folder", str(test_plugins), "--self-test", "--preset", str(out / "opacity.ffx"), "--out", str(local)]]:
        process = subprocess.run([str(out / diagnostic.name), *args], env=diagnostic_env, text=True, encoding="utf-8", capture_output=True, timeout=120)
        log.append(process.stdout + process.stderr)
        if process.returncode != 0:
            raise RuntimeError(f"portable native diagnostic failed: {log[-1]}")
    inventory = json.loads(passive.read_text(encoding="utf-8"))
    evidence = json.loads(local.read_text(encoding="utf-8"))
    assert inventory["executions"] == [] and len(inventory["scan"]["plugins"]) == 1
    assert inventory["scan"]["plugins"][0]["loaded"] is False
    assert evidence["sourceCommit"] == results["source_commit"]
    assert evidence["preset"]["decoded"]["effectcraftPreset"] == 1
    assert evidence["preset"]["appliedToNativePlugin"] is False
    assert len(evidence["executions"]) == 1 and evidence["executions"][0]["selfTestPassed"] is True
    execution = evidence["executions"][0]
    assert execution["checkpoint"]["identity"]["source"] == "PluginDataEntryFunction2"
    assert execution["checkpoint"]["identity"]["matchName"] == "org.effectcraft.test.gain"
    assert execution["checkpoint"]["nativeParameters"][0]["name"] == "Gain"
    assert isinstance(execution["checkpoint"]["nativeParameters"][0]["diskId"], int)
    assert execution["checkpoint"]["stage"] == "complete"
    # A native exit must retain the last entered stage and never claim a render.
    crash = out / "diagnostic-crash"
    crash.mkdir()
    (crash / "input.rgba32f").write_bytes(struct.pack("<16f", *([0.2, 0.3, 0.4, 1.0] * 4)))
    request = crash / "request.json"
    request.write_text(json.dumps({"protocol": 1, "operation": "diagnose", "plugin": str(plugin), "width": 2, "height": 2, "time": 0, "params": [[1, 4.0]]}), encoding="utf-8")
    response = crash / "response.json"
    result = subprocess.run([str(native / "effectcraft-aex-worker.exe"), str(request), str(response)], capture_output=True, timeout=60)
    checkpoint = json.loads(response.read_text(encoding="utf-8"))
    assert result.returncode == 71 and checkpoint["stage"] == "smartRender"
    assert checkpoint["loaded"] is True and checkpoint["initialized"] is True and checkpoint["rendered"] is False
    (out / "aex-crash-checkpoint.json").write_text(json.dumps(checkpoint, indent=2), encoding="utf-8")
    log.append("Portable diagnostic: passive discovery, exact registration identity, native disk ID, two changed frames, FFX preservation and exit-71 checkpoint passed.\n")
    (out / "aex_local_diagnostic.txt").write_text("\n".join(log), encoding="utf-8")
    results["aex_local_diagnostic"] = True
    results["target"] = "x86_64-pc-windows-msvc"
    (out / "WINDOWS-VALIDATION.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
