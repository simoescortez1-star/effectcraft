# effectcraft-ae-bridge (L3)

Bounded external AEP conversion and passive PE plugin inventory. No native AEX runtime,
FFX decoder, Adobe SDK bindings or third-party format decoder is linked into this crate.

See [the architecture, build instructions, matrix and limits](../../docs/after-effects-compatibility.md).
`process` is native-only; the byte inspector, configuration and report contract build on Wasm.
Header inspection follows Microsoft's public PE/COFF documentation. The external converter
has a separately pinned revision and provenance; it is not a workspace dependency.

Specification reference: Microsoft Learn **PE Format**, accessed 2026-10-15,
sections MS-DOS Stub, Signature, COFF File Header, Machine Types, Characteristics
and Optional Header. No native plugin implementation is consulted or executed.
