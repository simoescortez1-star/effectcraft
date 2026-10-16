# effectcraft-ae-bridge (L3)

Safe Rust, with no AEP decoder or native plugin ABI linked into the editor. Conversion
and native execution use bounded files and disposable helper processes. Discovery never
loads a DLL. A valid PE DLL is only a candidate, and a successful default-frame render
does not establish commercial preset compatibility.

See the [architecture, build instructions, compatibility matrix and limits](../../docs/after-effects-compatibility.md).
Process execution is native-only; passive byte inspection, configuration and the report
contract also build on WebAssembly. The external converter has a separately pinned revision
and provenance, and is not a workspace dependency.

`effectcraft-aex-diagnostic --help` describes the standalone local diagnostic. Its default
operation is passive inventory. `--plugin`, `--self-test` and exact PiPL
`--match-name ... --execute-matching` explicitly select execution targets. The generated
JSON preserves setup checkpoints, worker errors, native parameter declarations and frame
evidence. `--preset` decodes a user FFX through the separate converter without silently
applying unverified native mappings.

## Metadata format references and limits

The implemented subset uses the public [Microsoft PE/COFF specification](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format)
(online PE32+ edition, referenced 2026-10-16): section RVAs, data directories, import and
export tables, delay imports with RVA addressing and resource directories. Fixed version
information follows Microsoft's `VS_VERSIONINFO` / `VS_FIXEDFILEINFO` layout. PiPL property
records use a version/count header and vendor/key/id/length records, with bounded Pascal
or terminated strings; canonical big-endian and explicit Windows little-endian forms are
handled. The public [After Effects PiPL resource guide](https://github.com/docsforadobe/after-effects-plugin-guide/blob/master/docs/intro/pipl-resources.md)
(online guide read 2026-10-16) describes their identity/type role and multi-effect limits.
Unknown resource forms are reported, never inferred from DLL filenames or stray strings.
No commercial PiPL has been validated in this environment. Exact registration identity is
also obtained during explicit execution through `PluginDataEntryFunction2` in the external
host; that mechanism is exercised by the original Gain test plugin on Windows.

Tests generate their own PE/resource records in code; no Adobe SDK samples, commercial
plugins, user presets or private media are embedded. Parsing caps section counts, strings,
table entries, resource sizes and traversal depth. Import search results are hints rather
than a complete model of the Windows loader. See the edition's
[measured S_Shake report](../../packaging/after-effects/RELATORIO-S_Shake.md) for the exact
decoded user-preset data and the remaining native mapping, host-suite and fidelity gaps.

The pinned aexlo integration, FFI modifications and original redistributable test plugin
are built separately under `packaging/after-effects/native-worker`; their sources, bootstrap,
lockfile and notices are included in the source package. Process isolation and deadlines
contain failures but do not provide an operating-system security sandbox.
