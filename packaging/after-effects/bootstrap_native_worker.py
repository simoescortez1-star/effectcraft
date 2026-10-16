#!/usr/bin/env python3
"""Build the optional native ABI worker in its own pinned external workspace."""
import argparse
from pathlib import Path
import shutil
import subprocess

REVISION = "28e9f9a4a371d0ecae0d2b183e167755b3a7cac3"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--target", default="x86_64-pc-windows-msvc")
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    source = Path(__file__).resolve().parent
    work = options.work.resolve()
    if work == source or source in work.parents:
        parser.error("worker build directory must be outside the source tree")
    work.mkdir(parents=True, exist_ok=False)
    checkout = work / "aexlo"
    subprocess.run(["git", "clone", "--no-checkout", "--filter=blob:none", "https://github.com/potistudio/aexlo", str(checkout)], check=True)
    subprocess.run(["git", "-C", str(checkout), "checkout", "--detach", REVISION], check=True)
    # Bindgen represents PrPixelFormat as signed on Windows. These FourCC
    # values are bit patterns; match the host's u32 storage without a panic.
    path = checkout / "crates/aexlo/src/suites/pixel_format.rs"
    text = path.read_text(encoding="utf-8")
    for old, new in [
        ("i.add_supported_pixel_format(pixelFormat)", "i.add_supported_pixel_format(pixelFormat as u32)"),
        ("4 * depth.bytes(), pixelFormat)", "4 * depth.bytes(), pixelFormat as u32)"),
    ]:
        if text.count(old) != 1:
            raise RuntimeError("pinned aexlo Windows type anchor changed")
        text = text.replace(old, new)
    path.write_text(text, encoding="utf-8")
    worker = work / "worker"
    shutil.copytree(source / "native-worker", worker, ignore=shutil.ignore_patterns("target", "Cargo.lock"))
    path = worker / "Cargo.toml"
    text = path.read_text(encoding="utf-8")
    old = f'aexlo = {{ git = "https://github.com/potistudio/aexlo", rev = "{REVISION}" }}'
    if old not in text:
        raise RuntimeError("pinned worker dependency anchor changed")
    text = text.replace(old, f'aexlo = {{ path = "{(checkout / "crates/aexlo").as_posix()}" }}')
    path.write_text(text, encoding="utf-8")
    shutil.copyfile(source / "native-worker/Cargo.lock", worker / "Cargo.lock")
    subprocess.run(["cargo", "+1.99.0", "check" if options.check else "build", "--locked", "--release", "--workspace", "--manifest-path", str(path), "--target", options.target, "--target-dir", str(work / "target")], check=True)
    print(f"Native ABI worker built separately at {work / 'target' / options.target / 'release'}")


if __name__ == "__main__":
    main()
