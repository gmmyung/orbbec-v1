# Bundled SDK distribution

## Packages

| Crate | Version | Contents |
| --- | --- | --- |
| orbbec-v1-bin-x86_64 | 1.10.37 | Original x86_64 SDK libraries, config, notices, upstream sources |
| orbbec-v1-bin-aarch64 | 1.10.37 | Original AArch64 SDK libraries, config, notices, upstream sources |
| orbbec-v1-sys | 0.1.0 | Pregenerated target C bindings and SDK staging/linking |
| orbbec-v1 | 0.1.0 | Safe synchronous API and examples |
| orbbec-viewer | 0.1.0 | Native egui viewer |

The sys crate selects its binary dependency with Cargo target triples, pins
version `=1.10.37`, and copies the libraries into its own OUT_DIR. ELF machine
checks reject a mismatched library architecture. Loader aliases are created at
build time, avoiding duplicate copies of the large SDK file in the crate archive.
Cargo supplies the loader search path for run/test. No network downloads happen
in build scripts and no libclang is needed by consumers.

`ORBBEC_V1_SDK_DIR` overrides the selected installation at build time; it must
contain the same SDK version's four `lib/` files. The binary dependency remains
in Cargo's dependency graph. The override does not suppress its download.

## Reproducing the vendored payload

```bash
python3 scripts/fetch-sdk.py
python3 scripts/vendor-sdk.py
```

The scripts validate the official ZIP SHA-256 hashes in `scripts/sdk-lock.json`
and compare extracted library bytes with the hashed ZIP before staging them.
The binary crates contain no SDK examples, tools, OpenCV, or duplicated symlinks.
Each binary crate has `provenance.json`, including payload hashes, source URLs,
source hashes, and an explicit source-correspondence status.

Pregenerated Rust files were produced using bindgen 0.72.1 and the corresponding
v1.10.37 architecture headers. To regenerate, install libclang/target headers and
use the separate, unpublished maintainer tool, for example:

```bash
scripts/with-sdk.sh cargo run --manifest-path tools/generate-bindings/Cargo.toml \
  --target-dir target/generate-bindings -- x86_64-unknown-linux-gnu \
  .sdk/x64/OrbbecSDK_v1.10.37/SDK \
  crates/orbbec-v1-sys/bindings/x86_64-unknown-linux-gnu.rs
```

For AArch64, change the target, SDK directory, and output filename; supply
`ORBBEC_V1_SYSROOT` when generating on an x86_64 host. The local helper still
supports this machine's extracted Clang resource headers for maintainer use.

## Local verification

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
python3 scripts/check-distribution.py
# While reviewing uncommitted changes:
python3 scripts/check-distribution.py --allow-dirty
scripts/build-arm64.sh --viewer
```

The distribution check uses Cargo's workspace packaging support (validated with
Cargo 1.99.1), verifies package size and payload hashes, extracts the normalized
published manifests into a temporary directory outside the checkout, and runs
the safe crate's tests with SDK/Clang/loader environment overrides removed. It
also checks the dependency tree for each target. No publication takes place.
Older Cargo versions may require publishing the dependency packages in order
before packaging dependent crates against crates.io.

## Standalone runtime deployment

For a native viewer bundle:

```bash
python3 scripts/package-viewer.py
# Optional optimized build:
python3 scripts/package-viewer.py --release
```

The output is `target/dist/orbbec-viewer-ARCH/`, containing the executable and
`orbbec-sdk/{lib,config,sources,licenses}` plus original license, provenance, and
third-party notes. The executable has an `$ORIGIN/orbbec-sdk/lib` RPATH, so this
layout can be moved without setting `LD_LIBRARY_PATH`. This does not embed the
SDK into the executable. Compatible system libraries, desktop/OpenGL support,
and USB permissions remain target-machine requirements.

Library consumers should copy the directory returned by `sdk_directory()` into
their application distribution and configure their own executable loader path.
`cargo run -p orbbec-v1 --example sdk_path` prints the build's selected SDK staging
directory. Include the matching binary crate's notices and applicable source
materials when distributing an application. A build-time Cargo path is not a
portable installation location. ARM64 deployment needs ARM64 libraries and still
requires runtime validation on the target board.

## License and source status

The project owner elected to proceed with preparing redistribution of the SDK.
The SDK and third-party terms are preserved unchanged; this decision does not
relicense the native binaries or constitute permission from Orbbec. Rust code
retains the repository's MIT license. Binary crates use `license-file`, pointing
to the complete original SDK notice rather than labeling their binaries MIT.

Source inspection found a discrepancy: the original SDK notice lists live555
2019.08.13 under LGPL 2.1, whereas both shipped binaries contain 2022.04.26 and
upstream 2022.04.26 sources carry LGPL 3.0/GPL 3.0 license texts. Those upstream
texts are included separately and the original notice is retained. The x86_64
libusb binary reports 1.0.26.11791; upstream libusb 1.0.26 source and its LGPL
2.1 license are included.

The supplied archives are upstream sources for observed component versions,
not verified corresponding sources for vendor builds. Orbbec-specific patches
and build configurations have not been established. `source_correspondence_verified`
therefore remains false. Exact source correspondence and the live555 licensing
discrepancy must be resolved before claiming full LGPL compliance or publicly
publishing these binary packages. Other third-party source/notice obligations in
the complete SDK license also need to be honored for a public distribution.

No packages have been uploaded. The artifacts are local and reviewable. Once
public release requirements are resolved, publish in this order:

1. Both `orbbec-v1-bin-*` crates.
2. `orbbec-v1-sys`.
3. `orbbec-v1`.
4. `orbbec-viewer`, if desired.

Set project repository/documentation URLs when a public repository exists; none
is invented for the local repository.
