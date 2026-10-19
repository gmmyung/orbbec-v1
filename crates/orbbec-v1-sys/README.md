# orbbec-v1-sys

Raw C bindings to **Orbbec SDK 1.10.37** for Linux GNU x86_64 and AArch64.
Use `orbbec-v1` for the safe synchronous capture API.

The matching `orbbec-v1-bin-*` dependency supplies the original SDK shared
libraries and configuration. Bindings are pregenerated for both target triples;
building this crate requires neither libclang nor a separate SDK download.
Cargo builds perform no network requests beyond normal dependency fetching.

Set `ORBBEC_V1_SDK_DIR` to an existing SDK directory containing `lib/` and
optionally `config/` to override the bundled libraries. Every library is checked
for the target ELF architecture. Bindings remain pinned to SDK 1.10.37.

Cargo configures the runtime library path for `cargo run` and `cargo test`.
For a deployed application, copy the directory returned by `sdk_directory()`
and configure `LD_LIBRARY_PATH` or an executable RPATH. Do not depend on this
build-time directory existing on the deployment machine.

Rust binding code is MIT licensed. Native libraries retain the original SDK and
third-party licenses supplied by the binary packages. Their source provenance
and outstanding vendor-source verification are recorded in those packages.
