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
    ui_log_path = Path(os.environ["RUNNER_TEMP"]) / "quick-effects-ui.txt"
    ui_log = ui_log_path.read_text(encoding="utf-8-sig", errors="replace")
    ui_cases = [
        "ctrl_space_search_enter_applies_real_pixels_and_one_undo_redo_from_timeline_and_viewer",
        "arrows_select_current_results_escape_releases_focus_and_empty_selection_is_safe",
        "favorites_aliases_frequency_settings_and_custom_shortcut_survive_reload",
        "native_user_and_builtin_presets_use_existing_commands",
        "capture_png_and_clipboard_commands_render_the_composition",
        "console_respects_themes_dpi_settings_and_ime_composition",
        "configured_capture_folder_uses_scale_and_distinct_names_and_rejects_oversized_capture_before_rendering",
        "native_preset_reports_unavailable_effects_and_does_not_claim_empty_presets_were_applied",
    ]
    if "FAILED" in ui_log or not all(f"test {name} ... ok" in ui_log for name in ui_cases):
        raise RuntimeError("required Quick Effects UI regressions did not pass on this Windows build")
    (out / "quick-effects-ui.txt").write_text(ui_log, encoding="utf-8")
    results["quick_effects_console_ui"] = True
    results["quick_effects_console_ui_cases"] = len(ui_cases)

    for crate, module, test in [("effectcraft-engine", "tests_after_effects", "ffx_external_generated_end_to_end"),
                         ("effectcraft-engine", "tests_after_effects", "ffx_direct_transform_presets_render_and_preserve_saved_timebase"),
                         ("effectcraft-engine", "tests_ffx_native", "generated_native_ffx_cases_apply_render_export_persist_and_undo"),
                         ("effectcraft-engine", "tests_after_effects", "aex_native_worker_renders_and_recovers_after_child_exit"),
                         ("effectcraft-ui-egui", None, "ffx_console_uses_existing_importer_and_preserves_its_diagnostics")]:
        result = subprocess.run(["cargo", "+1.95.0", "test", "--locked", "--release", "--target", "x86_64-pc-windows-msvc",
                                 "-p", crate, *(["--lib", f"{module}::{test}"] if module else ["--test", "ui_quick_effects", test]), "--", "--ignored", "--exact", "--nocapture"],
                                cwd=root, env=env, text=True, encoding="utf-8", capture_output=True, errors="replace", timeout=900)
        log = result.stdout + result.stderr
        (out / f"{test}.txt").write_text(log, encoding="utf-8")
        print(log, flush=True)
        if result.returncode != 0 or "1 passed; 0 failed" not in log:
            raise RuntimeError(f"required native Windows render regression failed or did not run: {test}")
        results[test] = True
    cases = json.loads((out / "native-cases.json").read_text(encoding="utf-8"))
    generated_log = (out / "generated_native_ffx_cases_apply_render_export_persist_and_undo.txt").read_text(encoding="utf-8")
    if len(cases) < 15 or f"{len(cases)} original generated FFX cases verified" not in generated_log:
        raise RuntimeError("the required native FFX matrix did not run completely")
    results["generated_native_ffx_case_count"] = len(cases)
    results["generated_fixtures_authored_by_after_effects"] = False
    results["private_user_fixtures_tested_on_windows"] = False
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
