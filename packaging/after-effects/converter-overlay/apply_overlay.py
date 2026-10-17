"""Apply edition-owned extensions only to a pinned external checkout, never the editor."""
from pathlib import Path
import shutil


def apply(checkout):
    here = Path(__file__).resolve().parent
    checkout = Path(checkout)
    for name, old, new in [
        ("src/rifx.rs", "\nfn chunk<'a>(", "\npub(crate) fn chunk<'a>("),
        ("src/decode.rs", "\nfn nodes(", "\npub(crate) fn nodes("),
    ]:
        path = checkout / name
        text = path.read_text(encoding="utf-8")
        if text.count(old) != 1:
            raise RuntimeError(f"pinned reader anchor changed: {name}")
        path.write_text(text.replace(old, new), encoding="utf-8")
    shutil.copyfile(here / "reader_compat.rs", checkout / "src/effectcraft_compat.rs")
    path = checkout / "src/model.rs"
    text = path.read_text(encoding="utf-8")
    anchor = "pub struct Property {"
    if text.count(anchor) != 1:
        raise RuntimeError("pinned reader property model anchor changed")
    path.write_text(text.replace(anchor, anchor + "\n    pub source_ticks_per_second: Option<u32>,"), encoding="utf-8")
    path = checkout / "src/decode.rs"
    text = path.read_text(encoding="utf-8")
    anchor = '    let code = desc.and_then(|d| d.data.get(3)).copied();'
    if text.count(anchor) != 1:
        raise RuntimeError("pinned numeric clock anchor changed")
    text = text.replace(anchor, '''    let saved_rate = desc.and_then(|d| d.u32(12)).filter(|r| *r != 0);
    if saved_rate.is_some_and(|r| r > 1_000_000_000) {
        return Err(Error::InvalidFormat { offset: c.offset, message: "invalid property timebase" });
    }
    let ticks = saved_rate.unwrap_or(ticks);
''' + anchor)
    anchor = '    let mut p = Property {\n        storage_dimensions: dims,'
    if text.count(anchor) != 1:
        raise RuntimeError("pinned numeric property anchor changed")
    text = text.replace(anchor, '    let mut p = Property {\n        source_ticks_per_second: Some(ticks),\n        storage_dimensions: dims,')
    text = text.replace('    let mut p = Property {\n        storage_dimensions: None,', '    let mut p = Property {\n        source_ticks_per_second: Some(ticks),\n        storage_dimensions: None,')
    path.write_text(text, encoding="utf-8")
    path = checkout / "src/lib.rs"
    path.write_text(path.read_text(encoding="utf-8") + "\nmod effectcraft_compat;\npub use effectcraft_compat::effectcraft_preset;\n", encoding="utf-8")
    manifest = checkout / "examples/effectcraft/Cargo.toml"
    text = manifest.read_text(encoding="utf-8")
    manifest.write_text(text.replace('aep-reader = { path = "../.." }', 'aep-reader = { path = "../..", features = ["serde"] }'), encoding="utf-8")
    adapter = checkout / "examples/effectcraft/src/lib.rs"
    text = adapter.read_text(encoding="utf-8")
    anchor = 'pub fn import_with_options(bytes: &[u8], options: ImportOptions) -> Result<Import> {'
    if text.count(anchor) != 1:
        raise RuntimeError("pinned adapter entry anchor changed")
    text = text.replace(anchor, anchor + '\n    if bytes.get(8..12) == Some(b"FaFX") { return effectcraft_compat::preset(bytes); }')
    anchor = '    let factor = match match_name {'
    if text.count(anchor) != 1:
        raise RuntimeError("pinned property mapping clock anchor changed")
    text = text.replace(anchor, '    let ticks = source.source_ticks_per_second.unwrap_or(ticks);\n' + anchor)
    # AE saves 2D Position/Anchor/Scale vectors as two components in some presets.
    # Keep the target's existing third component instead of rejecting the vector.
    anchor = '        ([x, y, z], Value::Vec3(_)) => Ok(Value::Vec3([*x, *y, *z])),'
    if text.count(anchor) != 1:
        raise RuntimeError("pinned numeric vector anchor changed")
    text = text.replace(anchor, anchor + '\n        ([x, y], Value::Vec3([_, _, z])) => Ok(Value::Vec3([*x, *y, *z])),')
    text = text.replace('''                    // No active path: opening the ecproj must not resolve paths from the AEP.''', '''                    // Retain saved media references for the editor's explicit relinking workflow.''')
    text = text.replace('''                        missing: true,
                        width: f.and_then''', '''                        path: f.and_then(|f| f.path.clone()).unwrap_or_default(),
                        has_video: f.and_then(|f| f.path.as_deref()).and_then(|p| p.rsplit('.').next()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tif" | "tiff" | "webp" | "exr")),
                        missing: true,
                        width: f.and_then''')
    text = text.replace('''        let id = project.add_item(&item.name, Label::None, None, kind);''', '''        let fallback = match &kind {
            ItemKind::Footage(f) => f.path.rsplit(['/', '\\\\']).next().unwrap_or("Missing footage"),
            _ => "Untitled",
        };
        let name = if item.name.is_empty() { fallback } else { &item.name }.to_string();
        let id = project.add_item(&name, Label::None, None, kind);''')
    old = '''        // Only code 1 has independent authored-file evidence so far. Unknown
        // modes remain explicit approximations, while their saved ease is retained.
        if key.in_interpolation != 1 || key.out_interpolation != 1 {
            report.warn(context, format!("Unverified interpolation codes {}/{} at tick {}; interpolation is linear as a fallback.", key.in_interpolation, key.out_interpolation, key.time_ticks));
        }
'''
    if text.count(old) != 1:
        raise RuntimeError("pinned interpolation anchor changed")
    text = text.replace(old, "")
    anchor = '''                mapped.out_ease = out_ease;'''
    text = text.replace(anchor, anchor + '''
                let mode = |code, handles: &[effectcraft_keyframe::Ease]| match code {
                    1 => effectcraft_keyframe::Interp::Linear,
                    2 if !handles.is_empty() => effectcraft_keyframe::Interp::Bezier,
                    _ => effectcraft_keyframe::Interp::Linear,
                };
                mapped.in_interp = mode(key.in_interpolation, &mapped.in_ease);
                mapped.out_interp = mode(key.out_interpolation, &mapped.out_ease);
                if key.in_interpolation != 1 || key.out_interpolation != 1 {
                    report.warn(context, format!("Interpolation codes {}/{} at tick {}: code 2 with valid saved handles mapped to Bezier experimentally; other unknown codes use linear. AE visual equivalence is unverified.", key.in_interpolation, key.out_interpolation, key.time_ticks));
                }''')
    adapter.write_text(text + "\nmod effectcraft_compat;\n", encoding="utf-8")
    shutil.copyfile(here / "adapter_compat.rs", adapter.parent / "effectcraft_compat.rs")
    shutil.copyfile(here / "preset_reader_tests.rs", adapter.parent.parent / "tests/preset_reader.rs")
    # Upstream asserts the former linear fallback. Keep its time/value regression
    # and update the expected diagnostic for this edition's explicit Bezier map.
    path = checkout / "examples/effectcraft/tests/import.rs"
    text = path.read_text(encoding="utf-8")
    if text.count('w.message.contains("interpolation is linear")') != 1:
        raise RuntimeError("pinned interpolation regression anchor changed")
    path.write_text(text.replace('w.message.contains("interpolation is linear")', 'w.message.contains("mapped to Bezier experimentally")'), encoding="utf-8")
