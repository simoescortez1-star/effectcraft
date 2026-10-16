#!/usr/bin/env python3
"""Build a pinned, separate AEP converter. No decoder source enters EffectCraft."""
import argparse
from pathlib import Path
import shutil
import subprocess

REVISION = "16cfdb6983621370352a8dd26dcd01845378bb83"
REPOSITORY = "https://github.com/yueli-fx/aep-reader"


def run(*args, **kwargs):
    subprocess.run(args, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--effectcraft", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--work", type=Path, required=True, help="new directory outside the EffectCraft source tree")
    parser.add_argument("--lock-file", type=Path, help="use an existing separate converter lockfile without updating it")
    parser.add_argument("--target", help="explicit Rust target triple")
    parser.add_argument("--release", action="store_true", help="build and test the release profile")
    options = parser.parse_args()
    source = options.effectcraft.resolve()
    work = options.work.resolve()
    if source == work or source in work.parents:
        parser.error("external converter work directory must be outside the EffectCraft source tree")
    if not (source / "crates/project/Cargo.toml").is_file():
        parser.error("--effectcraft must point to the EffectCraft source tree")
    work.mkdir(parents=True, exist_ok=False)
    checkout = work / "aep-reader"
    run("git", "clone", "--no-checkout", "--filter=blob:none", REPOSITORY, str(checkout))
    run("git", "-C", str(checkout), "checkout", "--detach", REVISION)
    actual = subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True).strip()
    if actual != REVISION:
        raise RuntimeError("converter revision mismatch")
    manifest = checkout / "examples/effectcraft/Cargo.toml"
    text = manifest.read_text(encoding="utf-8")
    # Change only the five explicit sibling paths; package code stays external and pinned.
    for crate in ("project", "keyframe", "time", "color", "render"):
        old = f'../../../effectcraft/crates/{crate}'
        if old not in text:
            raise RuntimeError(f"missing expected upstream dependency path: {crate}")
        text = text.replace(old, (source / "crates" / crate).as_posix())
    manifest.write_text(text, encoding="utf-8")
    # Start from the editor's registry versions; Cargo records the separate converter graph.
    shutil.copyfile(options.lock_file or source / "Cargo.lock", manifest.parent / "Cargo.lock")
    arguments = ["--manifest-path", str(manifest), "--target-dir", str(work / "target")]
    if options.target:
        arguments.extend(["--target", options.target])
    if options.release:
        arguments.append("--release")
    run("cargo", "+1.95.0", "build", *(["--locked"] if options.lock_file else []), *arguments)
    run("cargo", "+1.95.0", "test", "--locked", *arguments)
    executable = work / "target"
    if options.target:
        executable /= options.target
    executable /= "release" if options.release else "debug"
    executable /= "aep-effectcraft"
    if __import__("os").name == "nt":
        executable = executable.with_suffix(".exe")
    print(f"External converter: {executable}")
    print("Set EFFECTCRAFT_AEP_CONVERTER to that absolute path before launching EffectCraft.")
    print("Review upstream PROVENANCE.md and licenses before redistributing this separate tool.")


if __name__ == "__main__":
    main()
