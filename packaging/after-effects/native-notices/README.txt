Optional native worker provenance

Host implementation: potistudio/aexlo, revision 28e9f9a4a371d0ecae0d2b183e167755b3a7cac3.
https://github.com/potistudio/aexlo/tree/28e9f9a4a371d0ecae0d2b183e167755b3a7cac3
MIT copyright notice: LICENSE-aexlo. This also covers its internal aexlo-macros and wrapper crates.
Edition changes to that external source: two explicit Windows FourCC signed-to-unsigned conversions.
The exact patch is in packaging/after-effects/bootstrap_native_worker.py in the source package.

The worker and original Gain test plugin are separate executables, outside the editor's
no-unsafe Rust workspace. Their own source contains no unsafe blocks; native FFI is in
external dependencies/macros. The editor never links aexlo, after-effects or after-effects-sys.
No Adobe SDK samples, artwork, commercial plugins or private user fixtures are distributed.

Registry dependencies and their licenses are recorded in third-party-notices/index.json.
colored is used unmodified under MPL-2.0. Its covered source is included in colored-source.zip.
Other licenses are selected from the options in each upstream Cargo manifest.
See UPSTREAM-LICENSE.txt where an upstream package declares a license but supplies no separate text.

Native loading is explicit. Process isolation is not an operating-system security sandbox.
Only run plugins from sources you trust and for which you have the necessary license.
