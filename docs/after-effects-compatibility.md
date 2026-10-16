# Experimental After Effects compatibility edition

This modified edition adds **external binary AEP import and passive AEX inventory**.
It does **not** meet the complete AEP + FFX + native AEX rendering milestone yet.
There is no claim of commercial-plugin compatibility or AE-equivalent visual fidelity.

## Source and provenance decisions

- Editor baseline: `0311bdc3` of EffectCraft, MIT OR Apache-2.0. Development branch:
  `feat/after-effects-compatibility`. No upstream code was replaced wholesale.
- [aep-reader](https://github.com/yueli-fx/aep-reader/tree/16cfdb6983621370352a8dd26dcd01845378bb83),
  revision `16cfdb6983621370352a8dd26dcd01845378bb83`, MIT OR Apache-2.0.
  Its [PROVENANCE.md](https://github.com/yueli-fx/aep-reader/blob/16cfdb6983621370352a8dd26dcd01845378bb83/PROVENANCE.md)
  explicitly disclaims independent clean-room provenance. Neither its decoder nor its
  experimental adapter is copied, vendored or linked into EffectCraft. An explicitly
  configured standalone executable exchanges files with the editor.
- [aexlo](https://github.com/potistudio/aexlo/tree/28e9f9a4a371d0ecae0d2b183e167755b3a7cac3),
  revision `28e9f9a4a371d0ecae0d2b183e167755b3a7cac3`, MIT, was evaluated through its manifest,
  license, public README and toolkit documentation. That revision requires Rust 1.99 and
  Windows x64/macOS arm64; the available toolchain is Rust 1.95 on Linux. Native loading
  also requires an FFI boundary, forbidden inside this workspace. It is not a dependency.
  A separately built Windows worker is the appropriate future integration point.
- No Adobe application internals, presets, SDK sample-plugin code, commercial binaries,
  activation mechanisms or reference artwork were incorporated. Restricted ArtCraft marks
  were removed for this modified distribution, as the upstream brand terms require.

## Build on Windows 11 x64

The original verified downloads are published in the fork's
[experimental release ae-compat-e30d8d6d](https://github.com/simoescortez1-star/effectcraft/releases/tag/ae-compat-e30d8d6d).
Extract `effectcraft-windows-x64-experimental.zip` completely and open `Launch.cmd`.
This is a portable ZIP, not an MSI installer. It includes the separate converter and notices;
it does not include commercial plugins. The original files were downloaded from the published
release and their SHA256 hashes matched the local packages. These original GNU binaries remain
cross-compiled and untested natively on Windows; publication does not change their validation.

The additional `.github/workflows/after-effects-downloads.yml` is configured to build the
editor/CLI and pinned converter natively on a Windows runner with MSVC and a static CRT,
test the AE bridge/engine and external adapter, smoke-test `effectcraft-cli --version`,
and upload Windows/source ZIPs, the report, metadata and checksums as an Actions artifact.
A subsequent job publishes a separate prerelease named `ae-native-<source SHA>`.
The workflow runs on pushes to the compatibility branch; its actual job results, rather
than its presence in the repository, establish whether a native build succeeded.

`package_downloads.py` requires the original compatibility commit to remain an ancestor,
checks all three PE32+ x64 files and dependency notice coverage, and includes the original
commit in the source bundle. Its license input is the original release ZIP pinned by SHA256;
dependency changes that need additional notices stop packaging. `RELATORIO-base-e30d8d6d.md`
is explicitly historical; `build.json` and `CLI-SMOKE.txt` record the new build's evidence.
Native GUI operation and fidelity on Windows 11 still require testing by the user.

Install Rust 1.95.0 with the MSVC x64 target, Visual Studio C++ Build Tools / Windows SDK,
Git and Python 3. The editor is still a Rust/egui application.

```powershell
rustup toolchain install 1.95.0 --component rustfmt --component clippy
rustup target add wasm32-unknown-unknown --toolchain 1.95.0
cargo +1.95.0 build --locked --release -p effectcraft -p effectcraft-cli --target x86_64-pc-windows-msvc
python packaging/after-effects/bootstrap_converter.py --work C:\EffectCraftTools\aep-bridge
$env:EFFECTCRAFT_AEP_CONVERTER = 'C:\EffectCraftTools\aep-bridge\target\debug\aep-effectcraft.exe'
.\target\x86_64-pc-windows-msvc\release\effectcraft.exe
```

Choose a **new**, writable `--work` directory outside the EffectCraft source tree. The
bootstrap script pins the external revision, adjusts only its sibling dependency paths,
builds it against this editor's model and runs its default tests. Keep that work directory's
Cargo.lock with the separate tool for reproducible rebuilds. It is not a native AEX worker.
To rebuild subsequently use `cargo +1.95.0 build --locked --manifest-path
C:\EffectCraftTools\aep-bridge\aep-reader\examples\effectcraft\Cargo.toml --target-dir
C:\EffectCraftTools\aep-bridge\target` (one shell command).

The added `after-effects-windows.yml` workflow can build and test editor/CLI/converter on
an actual Windows runner. Merely adding a workflow does not mean it has run or passed.
Native Linux builds cannot run on Windows. The downloadable experimental package was
cross-compiled on Linux for `x86_64-pc-windows-gnu`; its PE32+ x64 files were inspected,
but startup and rendering on Windows have not been tested.

## Import and recovery

1. Set `EFFECTCRAFT_AEP_CONVERTER` to an absolute path before starting the desktop app or CLI.
2. Use **File → Import → After Effects Project**, or drop an `.aep` onto the editor.
   Import merges into a folder; it does not replace the current project. A single undo removes
   the import; redo restores it. The project can then be saved/reopened as `.ecproj`.
3. Read the import report. Converter JSON plus the original source path and validation repairs is stored in the imported folder's comment
   and returned as `conversionReport` by `file.importAfterEffects`. Unknown source bytes
   remain in the original AEP, not in a lossless `.ecproj` representation. Keep the AEP.
4. Expressions on imported properties are retained but disabled. The converter uses target
   defaults or warnings for unsupported data. Model validation repairs are added to the report.
5. Footage placeholders are listed as missing. Select each placeholder and use the existing
   **Replace Footage → File** command (`file.replaceFootage`) to choose a real source.
   Original references are in the report. Automatic path reconstruction / bulk relinking of
   these empty-path placeholders is not implemented.

The external adapter currently handles a subset of comps/precomps, folders, transforms,
keyframes, interpolation, text, shapes, masks, parents, mattes, blend modes, cameras/lights
and limited Gaussian Blur. These are upstream capabilities, not a claim that this edition
has verified every mapping. The local end-to-end binary fixture checks composition size,
FPS, layer/property/key time, disabled expressions, diagnostics, source immutability and
reopening. A separate contract test checks a visible solid through the editor renderer.
Neither test establishes visual equivalence with After Effects.

Engine/control-channel commands:

```json
{"command":"file.importAfterEffects","params":{"path":"C:/work/project.aep"}}
{"command":"effect.aex.addFolder","params":{"folder":"C:/Plugins"}}
{"command":"effect.aex.scan","params":{"installed":true}}
{"command":"effect.aex.list","params":{}}
{"command":"effect.aex.enable","params":{"path":"C:/Plugins/example.aex","enabled":false}}
```

These objects are the `params` of a control-channel `engine.execute` request, or use the
same command ids with `effectcraft-cli exec`. `file.import` also routes AEP files to the bridge.

## Isolation and limits

`effectcraft-ae-bridge` is L3 and depends only on Serde/JSON and the standard library.
The engine (L4) owns commands and document validation; host (L4) supplies the external
converter; the egui UI (L5) exposes the same commands. Lower layers gain no UI dependency.

Conversion copies at most 256 MiB to a task-owned temporary AEP. The converter receives
only that snapshot and a fresh output path, never the original source path. Shells are not
used. Project/report limits are 128 MiB / 8 MiB; warnings are limited to 10,000. The worker
has a 120-second deadline, is killed/reaped on timeout or polling errors, and temporary files
are removed on normal error/success paths. Stderr is bounded to 1 MiB while running and the
failure message includes at most 8,192 characters. No session edit occurs before validation.

This is **process separation, not an OS sandbox**: the configured trusted converter retains
its OS account's permissions. It has no enforced memory/CPU quota, job-object child-process
containment or seccomp/AppContainer policy. Output-size polling is not a hard disk quota.
A sudden OS kill may leave task-owned temporary files. Conversion and directory scans are
currently synchronous; very slow storage may block the interface. These are explicit limits.

Plugin discovery opens file headers without calling LoadLibrary or plugin entry points.
It checks MZ, bounded PE offset, AMD64 machine, PE32+ magic and DLL characteristic against
[Microsoft PE/COFF documentation](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format).
Only 64 + 26 header bytes are read per candidate. Scan limits: 64 root folders, depth 16,
20,000 entries and 4,096 candidates. Discovered symlink entries are skipped and canonical
paths deduplicate directories. PE validity is only a preliminary filter; exports, PiPL,
ABI correctness and effect identity are not established.

## Compatibility matrix

“Untested” means no runtime execution evidence. A checkbox changes a saved availability
preference only. Native candidates are **never** registered as renderable effects.

| Feature / plugin | Read/discover | Load | Initialize | Parameters | Render | Visual fidelity |
|---|---|---|---|---|---|---|
| Binary AEP, synthetic upstream fixture | External import tested | n/a | n/a | One keyed opacity + expression tested | Separate converted-data solid test | Not compared to AE |
| AEP text/shapes/masks/cameras and other upstream mappings | Experimental external adapter | n/a | n/a | Not exhaustively tested here | Not verified here | Unverified |
| AEPX | Unsupported, explicit error | n/a | n/a | No | No | Unverified |
| FFX | Unsupported, explicit error | n/a | n/a | No | No | Unverified |
| Synthetic malformed `.aex` | Rejected without execution | Not attempted | Not attempted | Not attempted | Not attempted | Unverified |
| AMD64 PE32+ `.aex` | Passive candidate inspection only | Not implemented | Not implemented | Not implemented | Not implemented | Unverified |
| Boris FX Sapphire | No installed licensed test copy | Untested | Untested | Untested | Untested | Unverified |
| Boris FX Continuum/BCC | No installed licensed test copy | Untested | Untested | Untested | Untested | Unverified |
| RE:Vision Twixtor | No installed licensed test copy | Untested | Untested | Untested | Untested | Unverified |
| RE:Vision RSMB | No installed licensed test copy | Untested | Untested | Untested | Untested | Unverified |
| Red Giant Universe | No installed licensed test copy | Untested | Untested | Untested | Untested | Unverified |
| Video Copilot Twitch | No installed licensed test copy | Untested | Untested | Untested | Untested | Unverified |
| Deep Glow | No installed licensed test copy | Untested | Untested | Untested | Untested | Unverified |

**Animation → Apply After Effects Preset** and `.ffx` drops return an explicit unsupported
error before editing anything. There is no fake JSON payload renamed `.ffx`, fabricated preset
mapping or round-trip claim. The unknown FFX bytes are not parsed or rewritten.

## Validation and next steps

```sh
cargo test --locked -p effectcraft-ae-bridge
cargo test --locked -p effectcraft-engine --lib after_effects
cargo test --locked -p effectcraft-engine --lib import_project
cargo xtask layers
cargo xtask assets
cargo xtask ci
```

The ignored `aep_external_binary_end_to_end` test additionally requires
`EFFECTCRAFT_AEP_CONVERTER` and `EFFECTCRAFT_TEST_AEP` pointing to the explicitly supplied
upstream synthetic fixture, then run `cargo test -p effectcraft-engine --lib
aep_external_binary_end_to_end -- --ignored` (one command). It expects the upstream test
fixture with a 1920×1080, 30 fps Main composition and one keyed opacity at 0.5 seconds.
It does not accept an arbitrary production project as a reference fixture.

Remaining work, in dependency order:

1. Add openly licensed, authorized AEP/FFX reference fixtures and expected parameter/frame
   outputs. Extend AEP tests beyond the initial fixture and implement structured relink data.
2. Specify and implement only evidenced FFX property/keyframe structures, with unknown-byte
   retention and transactional application to selected layers. No such decoder exists here.
3. Build an independently audited Windows x64 AEX worker outside this no-unsafe workspace,
   with process containment, versioned IPC, explicit consent to execute, deadlines, shared
   frame transport and instance ids. Start with an original permissively licensed test DLL.
4. Add plugin parameter mapping and render hooks through the existing effects registry;
   test 8/16/32-bit channels, premultiplication, stride, failures and independent instances.
   Cache keys must include binary hash/version/instance/parameters/time/depth.
5. Only then test the prioritized commercial plugins using the user's installed, licensed
   copies. GPU suites, threading and multi-frame state need independent verification.

There is no native AEX render cache, GPU bridge, multi-instance ABI or commercial-plugin
license handling in this delivery. No activation/DRM bypass is attempted or needed.

For the local Linux validation run, `cargo xtask ci` additionally used
`CARGO_PROFILE_RELEASE_LTO=off`, `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16` and
`CARGO_BUILD_JOBS=2` to bound compilation/linking cost. All checks and test assertions
remain enabled. This CI profile does not establish performance of the normal release build;
the local downloadable Windows GNU build uses the same release profile overrides
for all dependency crates and executables (`LTO=off`, `CODEGEN_UNITS=16`).
No runtime-performance conclusion is drawn from compilation.

The final local CI run uses the environment's default headless configuration: GPU
comparisons skip through the existing test helpers when no adapter is available.
A separate run with a locally installed Mesa Vulkan software driver reached 210 passing
GPU tests and two failures (`tests_fx_pixel2::stylize` / HexTile and
`tests_fx_stylize::posterize_thresholds_strobe_vignette` / Threshold RGB). Those existing
effects were not changed in this edition. This is an unresolved GPU validation limitation,
not evidence of Windows GPU compatibility. The plugin-manager screenshot used that
software driver successfully; it does not validate the failing effects.
