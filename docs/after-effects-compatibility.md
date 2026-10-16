# Experimental After Effects compatibility edition

This edition adds external AEP import, partial binary FFX application and an optional
Windows AEX worker. It does not provide general AE or commercial-plugin compatibility.
The Portuguese [technical report](../packaging/after-effects/RELATORIO-avancado.md)
records the measured user fixtures, validation requirements and remaining limitations.

## Architecture and provenance

The editor keeps its safe-Rust workspace. The L3 `ae-bridge` crate exchanges bounded
files with separate executables and exposes native effects through the existing L2
`effects::plugin::EffectPlugin` interface. The L4 engine validates data and performs
transactional commands; the L5 egui UI uses those commands. No decoder or native ABI
library is linked into the editor.

- Original editor baseline: `0311bdc3`. Compatibility commit `e30d8d6d` is preserved
  as an ancestor. This stage uses `feat/ae-advanced-compatibility` in the user's fork.
- External reader: [aep-reader, 16cfdb6983621370352a8dd26dcd01845378bb83](https://github.com/yueli-fx/aep-reader/tree/16cfdb6983621370352a8dd26dcd01845378bb83),
  MIT OR Apache-2.0. Its provenance statement disclaims independent clean-room status.
  It remains external. Edition-owned overlays under `packaging/after-effects/converter-overlay`
  add FaFX settings extraction, data-only preset conversion, media references and
  experimental Bezier mapping. The bootstrap applies these only in an external checkout.
- External native host: [aexlo, 28e9f9a4a371d0ecae0d2b183e167755b3a7cac3](https://github.com/potistudio/aexlo/tree/28e9f9a4a371d0ecae0d2b183e167755b3a7cac3),
  MIT, built with Rust 1.99. Two explicit Windows FourCC casts are applied by the bootstrap.
  The worker, its dependency graph and original Gain test plugin have a separate Cargo
  workspace. Native FFI/unsafe resides in external dependencies/macros.
- No Adobe SDK sample code, commercial binaries, artwork, private media or user-created
  presets are redistributed. The user explicitly supplied their own FFX/AEP for analysis.
  Those fixtures stay private; public regression containers are generated from original code.
  Modified ArtCraft branding remains removed as in the compatibility base.

## Windows package and rebuild

The [original Release](https://github.com/simoescortez1-star/effectcraft/releases/tag/ae-compat-e30d8d6d)
remains available. Each successful native delivery workflow publishes a separate prerelease
`ae-native-<commit>`. Inspect the actual Actions result and `build.json`; the presence of a
workflow alone is not validation. Extract the complete portable ZIP and start `Launch.cmd`.
No commercial plugins or MSI installer are included.

For a source build install Visual Studio C++ Build Tools/Windows SDK, Git, Python,
Rust 1.95 with the Windows MSVC target, and Rust 1.99 for the separate native worker:

```powershell
cargo +1.95.0 build --locked --release --target x86_64-pc-windows-msvc -p effectcraft -p effectcraft-cli
python packaging/after-effects/bootstrap_converter.py --work C:\EffectCraftTools\aep --lock-file packaging/after-effects/converter-Cargo.lock --target x86_64-pc-windows-msvc --release
python packaging/after-effects/bootstrap_native_worker.py --work C:\EffectCraftTools\aex
$env:EFFECTCRAFT_AEP_CONVERTER = 'C:\EffectCraftTools\aep\target\x86_64-pc-windows-msvc\release\aep-effectcraft.exe'
$env:EFFECTCRAFT_AEX_WORKER = 'C:\EffectCraftTools\aex\target\x86_64-pc-windows-msvc\release\effectcraft-aex-worker.exe'
.\target\x86_64-pc-windows-msvc\release\effectcraft.exe
```

Each bootstrap needs a new writable directory outside the editor tree. Pinned revisions
and separate lockfiles are included. Use the native bootstrap for its required two-line
Windows dependency patch; building the unpatched aexlo revision directly fails type checking.
The delivery workflow builds with static CRT, LTO disabled and 16 codegen units, tests the
bridge/engine/converter and real Windows FFX/AEX render paths, verifies notice coverage,
checks PE32+ x64 images and smoke-tests the CLI before uploading/publishing ZIPs.
It rejects validation records from a different commit. The source ZIP includes the e30
history bundle and the binary ZIP includes native test logs and covered MPL source.

## AEP, FFX and media

Set the converter path before starting the editor, or use `Launch.cmd`.
**File → Import → After Effects Project** and AEP drops merge into a folder in one undo step.
The complete conversion report, validation repairs and original source path are saved in
that folder's comment. Unsupported AEP bytes remain in the original file: keep it.
Expressions are retained but disabled. AEPX remains unsupported.

The external adapter covers subsets of comps/precomps, folders, transforms, independent
position axes, time remap, text, shapes, masks, parents, mattes, blend modes, cameras/lights
and Gaussian Blur. This is not exhaustive validation. Code 2 with valid temporal handles
now uses experimental Bezier; unverified codes retain explicit warnings. Speed units,
spatial/roving semantics, 3D materials and other advanced mappings need AE references.

Original media paths are retained. Existing paths resolve through editor services; missing
items support **File → Relink Missing Footage**, recursive folder search, dry run, cancellation,
exact names and compatible source metadata. Windows backslashes are recognized across
platforms. Duplicate filenames never autoassign. Unsupported or incomplete metadata may
still require **Replace Footage → File**. Existing interpretation and sequence ranges persist.

**Animation → Apply After Effects Preset** and FFX drops use the same external process.
The FaFX parser extracts named settings blocks without executing expressions. Properties
support both direct `tdbs` storage and the `sspc` envelope, including nested
`tdsi` match-name paths and end-of-path markers. Display names do not select mappings.
Saved nonzero `beso`/`tdb4` clock fields must agree; conflicting clocks reject the preset.
The uploaded Scale regression uses 30720 ticks/s, rather than the former fixed 8000.
The engine also reveals applied properties in the timeline and reports actual property/key counts.
Recognized transform properties and the existing narrow Gaussian mapper supply a carrier project.
Two-component vectors retain the target's third component when mapped to a Vec3 property.
The engine applies only the explicit property map/effect groups, shifts keys to each target
layer's current time, assigns fresh IDs and supports undo/redo and reopening. Selection is
limited to 16 layers. Unknown effects are retained as decoded trees plus exact source hex
in the layer comment; the result distinguishes `applied` from `preserved`.

The supplied Sapphire `S_Shake` is **preserved only**, with 84 parameter children and its
saved animation. No Sapphire rendering was tested. It is not replaced by an approximate
shake. Automatic matching to commercial AEX parameter layouts is pending. Presets without
any saved clock retain an explicit 8000-tick fallback warning. Saved offsets are added to
the target layer's local current time; they are not rebased to their first key. Raw ticks,
clock provenance and bytes are preserved. AE application/curve equivalence remains unverified.
The FFX path is limited to 1 MiB input, 6 MiB decoded metadata, 8 MiB history per target
layer and 64 MiB preservation per command. AEP conversion retains 256 MiB input,
128 MiB project, 8 MiB report and 10,000-warning limits.

## Native AEX execution

Scanning only reads headers. It checks MZ, bounded PE offset, AMD64, PE32+ and DLL flags
against Microsoft's PE/COFF specification. It reads 64 + 26 header bytes and never calls
LoadLibrary. Limits: 64 folders, depth 16, 20,000 entries, 4096 candidates; symlinks are skipped.
PE validity establishes candidacy, not ABI/effect compatibility.

Add a folder in Plugin Manager, scan, then explicitly click **Load and test**. The
worker location is reported by `effect.aex.list`. **Scan bundled test plugin** discovers
the package's original Gain fixture without running it. **Load, test and apply** executes
the explicit real-frame check, then runs the ordinary `effect.apply` command on selected
layers. Repeating **Load and test** re-renders and returns a consistent status contract.
The separate worker loads the chosen library, runs initialization and parameter setup, and renders a
2×2 test frame. Only successful plugins enter the normal effect registry under
**Native AEX (experimental)**. Stage fields reflect observed execution; visual fidelity
remains unverified. The availability checkbox disables already loaded native effects too.
Opening a project does not automatically execute its plugins; manually load the same file
and location first. The current identity includes canonical path and binary content.

The initial subset is Windows x64 CPU Smart Pre-Render/Smart Render, Float Slider controls,
32-bit premultiplied RGBA, fixed geometry, at most 256 parameters, 4,194,304 pixels/frame
and 8192 per dimension. Native time is quantized to 1 ms; the complete source FPS context
is not passed. Missing suites, other parameter types, GPU, multiple inputs, UI,
audio and expansion of output bounds are unsupported. Per-frame fresh workers avoid shared
instance state and allow a later render after an exit/crash. They also cost startup time;
this is not a real-time performance claim. Native-affected layers/dependencies are not cached.
A failed render leaves input pixels intact and reports the failure in Plugin Manager/logs.

The original Gain fixture is redistributable. Apply Gain below 4 to change real pixels;
value 4 deliberately exits the worker for recovery testing. It is not a vendor effect.
All prioritized commercial plugins (Sapphire, BCC, Twixtor, RSMB, Universe, Deep Glow,
Twitch) remain untested. Nothing bypasses activation, DRM or license checks.

## Process limits and validation

Workers get task-owned files and structured arguments, never shell command strings.
Conversion snapshots the source so it cannot overwrite the original through the protocol.
Its default deadline is 120 seconds (configuration capped at 300); native requests have
10 seconds. Child processes are killed/reaped on timeout/error. Stderr/output are bounded
while running and when read back; native input/output is capped at 64 MiB each, JSON/log
at 1 MiB. Temporary directories are removed on normal exit/error.

This is process separation, **not an OS sandbox**: workers retain account permissions.
There is no enforced process memory quota, AppContainer/job-object descendant containment
or hard disk quota. Abrupt OS termination may leave temporary files. Conversion, scan and
native loading/rendering are synchronous; a slow plugin can block until its timeout.

```sh
cargo xtask ci
python packaging/after-effects/generate_fixtures.py --out /path/to/fixtures
# Set EFFECTCRAFT_AEP_CONVERTER and EFFECTCRAFT_TEST_FFX=/path/to/fixtures/opacity.ffx
cargo test --release -p effectcraft-engine --lib ffx_external_generated_end_to_end -- --ignored
cargo test --release -p effectcraft-engine --lib ffx_direct_transform_presets_render_and_preserve_saved_timebase -- --ignored
```

The private test `user_after_effects_inputs_preserve_measured_structure_and_unavailable_effect`
requires `EFFECTCRAFT_TEST_USER_AEP` and `EFFECTCRAFT_TEST_USER_FFX`; it expects the exact
user-supplied fixtures, not arbitrary projects. The older ignored AEP test expects its
1920×1080, 30 fps upstream synthetic fixture via `EFFECTCRAFT_TEST_AEP`.
The private `user_scale_ffx_creates_saved_keys_and_renders_animated_pixels` test needs
`EFFECTCRAFT_TEST_USER_SCALE` set to the uploaded Scale preset; the input is not redistributed.
`validate_windows.py` enforces both generated FFX tests and the native Gain test (pixels at
multiple times, worker exit/recovery, disable, undo, persistence) before delivery.

The standard CI headless environment may skip GPU comparisons without an adapter.
An earlier independent Mesa/llvmpipe run had 210 passes and two existing failures:
HexTile and ThresholdRGB. No shader changes are made here. Neither CI nor the Gain CPU
fixture establishes Windows GPU compatibility or AE visual equivalence.
