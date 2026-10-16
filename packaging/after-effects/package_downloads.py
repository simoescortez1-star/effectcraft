#!/usr/bin/env python3
"""Package a Windows build without losing the original AEP edition's history or notices."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import zipfile

BASE = "0311bdc34d0dc10c2d18ff7875b4f52d483e5e27"
COMPATIBILITY = "e30d8d6dc23af8d95c76072a6e693cbd42b398a9"
NOTICE_SHA256 = "cd5324bb1b41f5eac11ea84573ed6ca63b23b0909dd4d5ca62a3e3aca17213a6"


def command(root, *args):
    return subprocess.check_output(args, cwd=root, text=True).strip()


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as file:
        for part in iter(lambda: file.read(1024 * 1024), b""):
            h.update(part)
    return h.hexdigest()


def pe_x64(path):
    with path.open("rb") as file:
        dos = file.read(64)
        if len(dos) != 64 or dos[:2] != b"MZ":
            raise ValueError(f"not a PE image: {path}")
        offset = int.from_bytes(dos[60:64], "little")
        if offset > 1024 * 1024:
            raise ValueError(f"unexpected PE offset: {path}")
        file.seek(offset)
        header = file.read(26)
        if len(header) != 26 or header[:6] != b"PE\0\0\x64\x86" or header[24:26] != b"\x0b\x02":
            raise ValueError(f"not a PE32+ x64 executable: {path}")


def dependencies(root, manifest, target):
    metadata = json.loads(command(root, "cargo", "+1.95.0", "metadata", "--locked", "--format-version", "1", "--filter-platform", target,
                                  "--manifest-path", str(manifest)))
    return {f"{p['name']}-{p['version']}" for p in metadata["packages"] if p.get("source")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--converter-work", type=Path, required=True)
    parser.add_argument("--notice-package", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--target", choices=["x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"], default="x86_64-pc-windows-msvc")
    parser.add_argument("--editor-bin", type=Path, help="override the editor/CLI release directory")
    parser.add_argument("--skip-native-smoke", action="store_true", help="offline package verification only; records native smoke as false")
    options = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    out = options.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    converter_work = options.converter_work.resolve()
    converter_root = converter_work / "aep-reader"
    converter_manifest = converter_root / "examples/effectcraft/Cargo.toml"
    bins = options.editor_bin or root / "target" / options.target / "release"
    executable_paths = {name: bins / name for name in ["effectcraft.exe", "effectcraft-cli.exe"]}
    executable_paths["aep-effectcraft.exe"] = converter_work / "target" / options.target / "release/aep-effectcraft.exe"
    for path in executable_paths.values():
        pe_x64(path)
    if digest(options.notice_package) != NOTICE_SHA256:
        raise ValueError("notice source package differs from the verified original distribution")
    required = dependencies(root, root / "Cargo.toml", options.target) | dependencies(root, converter_manifest, options.target)
    with zipfile.ZipFile(options.notice_package) as notices:
        index = json.loads(notices.read("third-party-notices/index.json"))
        extra_root = Path(__file__).parent / "additional-notices"
        extra_index = json.loads((extra_root / "index.json").read_text(encoding="utf-8"))
        if index.keys() & extra_index.keys():
            raise ValueError("additional notice index overlaps the original distribution")
        index.update(extra_index)
        missing = sorted(required - index.keys())
        if missing:
            raise ValueError(f"dependency notices must be refreshed before distribution: {missing}")
        for key in required:
            if not index[key]["notices"]:
                raise ValueError(f"no notice files recorded for {key}")
            for name in index[key]["notices"]:
                if key in extra_index:
                    if not (extra_root / key / name).is_file():
                        raise ValueError(f"missing additional notice for {key}: {name}")
                else:
                    notices.getinfo(f"third-party-notices/{key}/{name}")
        smoke = "Native CLI smoke test skipped (offline verification only)."
        if not options.skip_native_smoke:
            result = subprocess.run([str(executable_paths["effectcraft-cli.exe"].resolve()), "--version"], check=True,
                                    capture_output=True, text=True, errors="replace", timeout=30)
            smoke = result.stdout + result.stderr
        sha = command(root, "git", "rev-parse", "HEAD")
        subprocess.run(["git", "merge-base", "--is-ancestor", COMPATIBILITY, sha], cwd=root, check=True)
        build = {"source_commit": sha, "preserved_compatibility_commit": COMPATIBILITY, "target": options.target,
                 "rust": command(root, "rustc", "+1.95.0", "--version"), "native_cli_smoke_passed": not options.skip_native_smoke,
                 "windows_11_gui_tested": False, "ffx_supported": False, "aepx_supported": False, "aex_runtime_supported": False,
                 "executables": {name: {"bytes": path.stat().st_size, "sha256": digest(path)} for name, path in executable_paths.items()}}
        (out / "build.json").write_text(json.dumps(build, indent=2) + "\n", encoding="utf-8")
        (out / "CLI-SMOKE.txt").write_text(smoke + "\n", encoding="utf-8")
        (out / "RELATORIO.md").write_bytes((root / "packaging/after-effects/RELATORIO-base-e30d8d6d.md").read_bytes())
        readme = """EFFECTCRAFT - EDICAO EXPERIMENTAL E PARCIAL
Descompacte o ZIP Windows completamente em uma pasta gravavel e abra Launch.cmd.
Pacote portatil: nao requer MSI nem privilegios de administrador. Nao inclui plugins comerciais.
AEP usa o conversor separado incluido. FFX, AEPX e execucao/renderizacao AEX nao estao implementados.
Leia build.json e CLI-SMOKE.txt para a validacao desta compilacao.
RELATORIO.md descreve a validacao historica da base e30d8d6d, incluindo duas falhas GPU Mesa.
O funcionamento da interface e a fidelidade de renderizacao no Windows 11 ainda precisam de teste.
Os executaveis experimentais nao possuem assinatura de codigo.
"""
        (out / "LEIA-ME.txt").write_text(readme, encoding="utf-8")
        windows = out / "effectcraft-windows-x64-experimental.zip"
        with zipfile.ZipFile(windows, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as package:
            for name, path in executable_paths.items():
                package.write(path, name)
            for name in ["LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md"]:
                package.write(root / name, name)
            for name in notices.namelist():
                if name != "third-party-notices/index.json" and name.startswith(("third-party-notices/", "asset-notices/", "converter-notices/")):
                    package.writestr(name, notices.read(name))
            package.writestr("third-party-notices/index.json", json.dumps(index, indent=2) + "\n")
            for key, row in extra_index.items():
                for name in row["notices"]:
                    package.write(extra_root / key / name, f"third-party-notices/{key}/{name}")
            for name in ["build.json", "CLI-SMOKE.txt", "RELATORIO.md", "LEIA-ME.txt"]:
                package.write(out / name, name)
            package.write(root / "docs/after-effects-compatibility.md", "ARCHITECTURE-AND-COMPATIBILITY.md")
            package.write(root / "packaging/windows/portable.txt", "portable.txt")
            package.writestr("Launch.cmd", '@echo off\r\nset "EFFECTCRAFT_AEP_CONVERTER=%~dp0aep-effectcraft.exe"\r\n"%~dp0effectcraft.exe" %*\r\n')
        source = out / "effectcraft-source.zip"
        with source.open("wb") as file:
            subprocess.run(["git", "archive", "--format=zip", "--prefix=effectcraft/", "HEAD"], cwd=root, stdout=file, check=True)
        with tempfile.TemporaryDirectory(prefix="effectcraft-package-") as temp:
            bundle = Path(temp) / "effectcraft-changes.bundle"
            subprocess.run(["git", "bundle", "create", str(bundle), f"{BASE}..HEAD"], cwd=root, check=True)
            subprocess.run(["git", "bundle", "verify", str(bundle)], cwd=root, check=True)
            with zipfile.ZipFile(source, "a", compression=zipfile.ZIP_DEFLATED) as package:
                package.write(bundle, "delivery/history/effectcraft-changes.bundle")
                package.writestr("delivery/history/COMMITS.txt", command(root, "git", "log", "--format=fuller", "--stat", f"{BASE}..HEAD") + "\n")
                package.writestr("delivery/history/BASE.txt", f"Bundle prerequisite: {BASE}\nOriginal compatibility commit: {COMPATIBILITY}\n")
                for name in ["RELATORIO.md", "build.json", "CLI-SMOKE.txt", "LEIA-ME.txt"]:
                    package.write(out / name, "delivery/" + name)
        for path in [windows, source]:
            with zipfile.ZipFile(path) as package:
                if package.testzip() is not None:
                    raise ValueError(f"corrupt package: {path}")
        files = [windows, source] + [out / name for name in ["RELATORIO.md", "build.json", "CLI-SMOKE.txt", "LEIA-ME.txt"]]
        (out / "SHA256SUMS.txt").write_text("".join(f"{digest(path)}  {path.name}\n" for path in files), encoding="utf-8")
        print(json.dumps(build, indent=2))


if __name__ == "__main__":
    main()
