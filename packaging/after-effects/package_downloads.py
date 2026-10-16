#!/usr/bin/env python3
"""Package a Windows build without losing the original AEP edition's history or notices."""
import argparse
import hashlib
import json
import io
from pathlib import Path
import subprocess
import tempfile
import zipfile

BASE = "0311bdc34d0dc10c2d18ff7875b4f52d483e5e27"
COMPATIBILITY = "e30d8d6dc23af8d95c76072a6e693cbd42b398a9"
NOTICE_SHA256 = "cd5324bb1b41f5eac11ea84573ed6ca63b23b0909dd4d5ca62a3e3aca17213a6"


def command(root, *args):
    return subprocess.check_output(args, cwd=root, text=True, encoding="utf-8").strip()


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


def dependencies(root, manifest, target, toolchain="1.95.0"):
    metadata = json.loads(command(root, "cargo", f"+{toolchain}", "metadata", "--locked", "--format-version", "1", "--filter-platform", target,
                                  "--manifest-path", str(manifest)))
    return {f"{p['name']}-{p['version']}" for p in metadata["packages"] if p.get("source")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--converter-work", type=Path, required=True)
    parser.add_argument("--native-work", type=Path, required=True)
    parser.add_argument("--validation", type=Path, required=True)
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
    executable_paths = {"CortezEffects.exe": bins / "effectcraft.exe", **{name: bins / name for name in ["effectcraft-cli.exe", "effectcraft-aex-diagnostic.exe"]}}
    executable_paths["aep-effectcraft.exe"] = converter_work / "target" / options.target / "release/aep-effectcraft.exe"
    native_work = options.native_work.resolve()
    native_bins = native_work / "target" / options.target / "release"
    native_manifest = native_work / "worker/Cargo.toml"
    executable_paths["effectcraft-aex-worker.exe"] = native_bins / "effectcraft-aex-worker.exe"
    executable_paths["test-plugins/EffectCraft Gain.aex"] = native_bins / "effectcraft_aex_gain_fixture.dll"
    for path in executable_paths.values():
        pe_x64(path)
    if digest(options.notice_package) != NOTICE_SHA256:
        raise ValueError("notice source package differs from the verified original distribution")
    required = dependencies(root, root / "Cargo.toml", options.target) | dependencies(root, converter_manifest, options.target) | dependencies(root, native_manifest, options.target, "1.99.0")
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
        validation = json.loads((options.validation / "WINDOWS-VALIDATION.json").read_text(encoding="utf-8"))
        if validation.get("source_commit") != sha or not all(validation.get(test) is True for test in ["ffx_external_generated_end_to_end", "ffx_direct_transform_presets_render_and_preserve_saved_timebase", "aex_native_worker_renders_and_recovers_after_child_exit", "aex_local_diagnostic"]):
            raise ValueError("required Windows render validation is missing or belongs to another source commit")
        subprocess.run(["git", "merge-base", "--is-ancestor", COMPATIBILITY, sha], cwd=root, check=True)
        version_info = {}
        if not options.skip_native_smoke:
            editor_literal = str(executable_paths["CortezEffects.exe"].resolve()).replace("'", "''")
            ps = f"$v=(Get-Item -LiteralPath '{editor_literal}').VersionInfo; @{{product=$v.ProductName; description=$v.FileDescription; originalFilename=$v.OriginalFilename}} | ConvertTo-Json -Compress"
            version_info = json.loads(command(root, "powershell", "-NoProfile", "-NonInteractive", "-Command", ps))
            if version_info.get("product") != "CortezEffects" or version_info.get("originalFilename") != "CortezEffects.exe":
                raise ValueError("Windows executable resources do not carry the CortezEffects identity")
        build = {"product": "CortezEffects", "windows_version_info": version_info, "source_commit": sha, "preserved_compatibility_commit": COMPATIBILITY, "target": options.target,
                 "rust": command(root, "rustc", "+1.95.0", "--version"), "native_cli_smoke_passed": not options.skip_native_smoke,
                 "windows_11_gui_tested": False, "ffx_supported": "partial-mapped-properties", "aepx_supported": False, "aex_runtime_supported": "experimental-cpu-smart-render-float-sliders",
                 "native_validation": validation,
                 "executables": {name: {"bytes": path.stat().st_size, "sha256": digest(path)} for name, path in executable_paths.items()}}
        (out / "build.json").write_text(json.dumps(build, indent=2) + "\n", encoding="utf-8")
        (out / "CLI-SMOKE.txt").write_text(smoke + "\n", encoding="utf-8")
        sapphire_report = (root / "packaging/after-effects/RELATORIO-S_Shake.md").read_bytes()
        (out / "SAPPHIRE-S_SHAKE.md").write_bytes(sapphire_report)
        delivery = (f"\n\n## Identificação da compilação Windows\n\nCommit: `{sha}`. Alvo: `{options.target}`.\n\n"
                    f"Destino de publicação configurado no workflow: https://github.com/simoescortez1-star/effectcraft/releases/tag/cortezeffects-glass-{sha[:12]}\n\n"
                    "O endereço é publicado após o sucesso do pacote e sua disponibilidade é verificada na entrega.\n\n"
                    f"Validação automatizada desta compilação:\n\n```json\n{json.dumps(validation, indent=2)}\n```\n"
                    "\nWindows 11 com interação manual: não testado. Sapphire comercial: compatibilidade não comprovada.\n")
        (out / "RELATORIO.md").write_bytes((root / "docs/cortezeffects-redesign.md").read_bytes() + delivery.encode("utf-8"))
        readme = """CORTEZEFFECTS - LIQUID GLASS - EDICAO EXPERIMENTAL
Abra CortezEffects.exe ou Launch.cmd. Interface: Aparencia e personalizacao pelo botao de brilho no topo.
Cinco temas, cores de destaque, controles de vidro, wallpaper PNG/JPEG, fontes e movimento reduzido.
As preferencias e projetos antigos sao mantidos; a pasta de dados continua EffectCraftData por compatibilidade.
Vidro simulado com fundo desfocado em cache; nao utiliza Mica/Acrylic nem altera os pixels exportados.
Descompacte o ZIP Windows completamente em uma pasta gravavel e abra Launch.cmd.
Pacote portatil: nao requer MSI nem privilegios de administrador. Nao inclui plugins comerciais.
AEP e FFX usam o conversor separado incluido. Suporte parcial; AEPX ainda indisponivel.
FFX: propriedades reconhecidas sao aplicadas; efeitos indisponiveis ficam preservados como dados.
Scale em tdbs direto e clocks salvos agora sao lidos. A timeline revela as propriedades aplicadas.
No Scale anexado, as chaves ficam 38/60 s e 53/60 s apos o tempo atual; intervalo de 0,25 s.
O preset S_Shake enviado ainda exige um runtime Sapphire validado; nao ha substituicao visual.
Novo: effectcraft-aex-diagnostic.exe e Diagnosticar-Sapphire.cmd produzem log JSON local.
Abra Diagnosticar-Sapphire.cmd para testar o Gain e os modulos cujo PiPL declara S_Shake.
Arraste o seu FFX para Diagnosticar-Sapphire.cmd para incluir a analise de parametros/chaves.
Consulte SAPPHIRE-S_SHAKE.md para selecionar explicitamente um AEX sem PiPL reconhecido.
O diagnostico tenta valores padrao; nao aplica automaticamente o preset Sapphire nem comprova compatibilidade.
AEX: Plugin Manager > Add folder > Rescan > Load and test executa explicitamente o plugin.
Para testar, selecione uma camada, use Localizar plugin de teste incluido e Carregar, testar e aplicar.
EffectCraft Gain.aex tambem pode ser aplicado por Effects & Presets > Native AEX (experimental).
Somente CPU Smart Render, sliders float e frames de ate 4.194.304 pixels estao habilitados.
O plugin Gain e original e redistribuivel; nao representa compatibilidade com plugins comerciais.
Leia build.json e CLI-SMOKE.txt para a validacao desta compilacao.
RELATORIO.md descreve resultados, limites e pendencias desta etapa.
O funcionamento da interface e a fidelidade de renderizacao no Windows 11 ainda precisam de teste.
Os executaveis experimentais nao possuem assinatura de codigo.
"""
        (out / "LEIA-ME.txt").write_text(readme, encoding="utf-8")
        windows = out / "CortezEffects-Windows-x64.zip"
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
            for name in ["build.json", "CLI-SMOKE.txt", "RELATORIO.md", "SAPPHIRE-S_SHAKE.md", "LEIA-ME.txt"]:
                package.write(out / name, name)
            for path in sorted(options.validation.glob("*.txt")):
                package.write(path, "validation/" + path.name)
            package.write(options.validation / "WINDOWS-VALIDATION.json", "validation/WINDOWS-VALIDATION.json")
            for name in ["aex-local-diagnostic.json", "aex-crash-checkpoint.json"]:
                package.write(options.validation / name, "validation/" + name)
            for name in ["RELATORIO-base-e30d8d6d.md", "RELATORIO-avancado.md"]:
                package.write(root / "packaging/after-effects" / name, name)
            for path in (root / "packaging/after-effects/native-notices").iterdir():
                package.write(path, "native-worker-notices/" + path.name)
            # Ship the unmodified covered source for the MPL terminal-color crate.
            native_metadata = json.loads(command(root, "cargo", "+1.99.0", "metadata", "--locked", "--format-version", "1", "--manifest-path", str(native_manifest)))
            colored = next(p for p in native_metadata["packages"] if p["name"] == "colored")
            source_root = Path(colored["manifest_path"]).parent
            source_zip = io.BytesIO()
            with zipfile.ZipFile(source_zip,"w",compression=zipfile.ZIP_DEFLATED) as archive:
                for path in source_root.rglob("*"):
                    if path.is_file() and (path.suffix in [".rs", ".toml", ".md"] or path.name.startswith(("LICENSE", "COPYING"))):
                        archive.write(path, f"colored-{colored['version']}/" + path.relative_to(source_root).as_posix())
            package.writestr("native-worker-notices/colored-source.zip", source_zip.getvalue())
            package.write(root / "docs/after-effects-compatibility.md", "ARCHITECTURE-AND-COMPATIBILITY.md")
            package.write(root / "packaging/windows/portable.txt", "portable.txt")
            for path in (root / "assets/cortezeffects").iterdir():
                if path.is_file():
                    package.write(path, "asset-notices/cortezeffects/" + path.name)
            package.writestr("Launch.cmd", '@echo off\r\nset "EFFECTCRAFT_AEP_CONVERTER=%~dp0aep-effectcraft.exe"\r\nset "EFFECTCRAFT_AEX_WORKER=%~dp0effectcraft-aex-worker.exe"\r\n"%~dp0CortezEffects.exe" %*\r\n')
            package.write(root / "packaging/after-effects/Diagnosticar-Sapphire.cmd", "Diagnosticar-Sapphire.cmd")
        source = out / "CortezEffects-source.zip"
        with source.open("wb") as file:
            subprocess.run(["git", "archive", "--format=zip", "--prefix=CortezEffects/", "HEAD"], cwd=root, stdout=file, check=True)
        with tempfile.TemporaryDirectory(prefix="effectcraft-package-") as temp:
            bundle = Path(temp) / "effectcraft-changes.bundle"
            subprocess.run(["git", "bundle", "create", str(bundle), f"{BASE}..HEAD"], cwd=root, check=True)
            subprocess.run(["git", "bundle", "verify", str(bundle)], cwd=root, check=True)
            with zipfile.ZipFile(source, "a", compression=zipfile.ZIP_DEFLATED) as package:
                package.write(bundle, "delivery/history/effectcraft-changes.bundle")
                package.writestr("delivery/history/COMMITS.txt", command(root, "git", "log", "--format=fuller", "--stat", f"{BASE}..HEAD") + "\n")
                package.writestr("delivery/history/BASE.txt", f"Bundle prerequisite: {BASE}\nOriginal compatibility commit: {COMPATIBILITY}\n")
                for name in ["RELATORIO.md", "SAPPHIRE-S_SHAKE.md", "build.json", "CLI-SMOKE.txt", "LEIA-ME.txt"]:
                    package.write(out / name, "delivery/" + name)
        for path in [windows, source]:
            with zipfile.ZipFile(path) as package:
                if package.testzip() is not None:
                    raise ValueError(f"corrupt package: {path}")
        files = [windows, source] + [out / name for name in ["RELATORIO.md", "SAPPHIRE-S_SHAKE.md", "build.json", "CLI-SMOKE.txt", "LEIA-ME.txt"]]
        (out / "SHA256SUMS.txt").write_text("".join(f"{digest(path)}  {path.name}\n" for path in files), encoding="utf-8")
        print(json.dumps(build, indent=2))


if __name__ == "__main__":
    main()
