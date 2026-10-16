"""Edition-owned, checked patch to the pinned external ABI host (never the editor).

Expose the plugin's own registration identity and separate setup stages. No filename
heuristic or replacement effect implementation is involved.
"""
from pathlib import Path


def apply(checkout: Path):
    path = checkout / "crates/aexlo/src/instance.rs"
    text = path.read_text(encoding="utf-8")

    def replace(old, new):
        nonlocal text
        if text.count(old) != 1:
            raise RuntimeError(f"pinned native identity/stage anchor changed: {old[:80]}")
        text = text.replace(old, new)

    replace("struct PluginDataInfo {\n\tentry_point_name: Option<String>,\n}", """struct PluginDataInfo {
\tentry_point_name: Option<String>,
\tname: Option<String>,
\tmatch_name: Option<String>,
\tcategory: Option<String>,
\tkind: i32,
}

// Native pointers are confined to this external ABI boundary and disposable worker.
// Read at most 1024 bytes of each registration string, including its terminator.
unsafe fn edition_registration_string(ptr: *const u8) -> Option<String> {
\tif ptr.is_null() { return None; }
\tlet mut bytes = Vec::new();
\tfor i in 0..1024 {
\t\tlet byte = unsafe { *ptr.add(i) };
\t\tif byte == 0 { return String::from_utf8(bytes).ok(); }
\t\tbytes.push(byte);
\t}
\tNone
}""")
    replace("\tinfo.entry_point_name = Some(name.to_string_lossy().into_owned());", """\tinfo.entry_point_name = Some(name.to_string_lossy().into_owned());
\tinfo.name = unsafe { edition_registration_string(_in_name.cast()) };
\tinfo.match_name = unsafe { edition_registration_string(_in_match_name.cast()) };
\tinfo.category = unsafe { edition_registration_string(_in_category.cast()) };
\tinfo.kind = _in_kind;""")
    replace("\tentry_point_name: Option<String>,\n\tbinary_file_path: PathBuf,", "\tentry_point_name: Option<String>,\n\tedition_registration: Option<PluginDataInfo>,\n\tedition_version: u32,\n\tbinary_file_path: PathBuf,")
    replace("\t\t\t\tentry_point_name: None,", "\t\t\t\tentry_point_name: None,\n\t\t\t\tedition_registration: None,\n\t\t\t\tedition_version: 0,")
    replace("\tfn query_declared_entry_point(lib: &Library, pica: &after_effects_sys::SPBasicSuite) -> Option<String> {", "\tfn query_declared_entry_point(lib: &Library, pica: &after_effects_sys::SPBasicSuite) -> Option<PluginDataInfo> {")
    replace("\t\tinfo.entry_point_name\n\t}", "\t\tSome(info)\n\t}")
    replace("\t) -> Result<(PluginEntryPoint, String)> {", "\t) -> Result<(PluginEntryPoint, String, Option<PluginDataInfo>)> {")
    replace("\t\tlet candidates = declared\n\t\t\t.as_deref()", "\t\tlet candidates = declared.as_ref().and_then(|d| d.entry_point_name.as_deref())")
    replace("return Ok((symbol, candidate.to_string()));", "return Ok((symbol, candidate.to_string(), declared));")
    replace("let (entry_point, resolved_name) = Self::resolve_entry_point(&lib, self.pica.as_ref())?;", "let (entry_point, resolved_name, registration) = Self::resolve_entry_point(&lib, self.pica.as_ref())?;\n\t\tself.edition_registration = registration;")
    replace("\t\tself.global_out_flags = self.out_data.out_flags;", "\t\tself.global_out_flags = self.out_data.out_flags;\n\t\tself.edition_version = self.out_data.my_version;")
    replace("impl PluginInstance {\n\t/// Run the post-load", """impl Host {
\t/// Open and resolve only; initialization is explicitly staged by the worker.
\tpub fn edition_open(self, path: impl AsRef<Path>) -> Result<PluginInstance> {
\t\tlet mut instance = PluginInstance::new(path.as_ref());
\t\tinstance.load()?;
\t\tOk(instance)
\t}
}

impl PluginInstance {
\tpub fn edition_registration(&self) -> Option<(&str, &str, &str, i32)> {
\t\tlet d = self.edition_registration.as_ref()?;
\t\tSome((d.name.as_deref()?, d.match_name.as_deref()?, d.category.as_deref()?, d.kind))
\t}
\tpub fn edition_entry_point(&self) -> Option<&str> { self.entry_point_name.as_deref() }
\tpub fn edition_version(&self) -> u32 { self.edition_version }
\tpub fn edition_global_setup(&mut self) -> Result<()> { self.setup_global() }
\tpub fn edition_params_setup(&mut self) -> Result<()> { self.setup_params() }
\tpub fn edition_sequence_setup(&mut self) -> Result<()> {
\t\tself.setup_arbitrary_params();
\t\tself.setup_sequence()
\t}
\tpub fn edition_param_id(&self, index: usize) -> Option<i32> {
\t\tlet param = self.params.get(index)?;
\t\tSome(unsafe { param.uu.id })
\t}

\t/// Run the post-load""")
    path.write_text(text, encoding="utf-8")
