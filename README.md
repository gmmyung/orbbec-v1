# orbbec-v1-rs

Rust bindings for **Orbbec SDK v1.10.37**, initially developed with an **Astra
Stereo S U3** (`SV1301S_U3`, firmware `RD3013`). Targets are Linux GNU x86_64 and
AArch64. This is a local development repository; hardware results are recorded
in [docs/validation.md](docs/validation.md).

The workspace contains `orbbec-v1-sys` (generated C FFI) and `orbbec-v1` (a safe,
synchronous wrapper). The safe API covers enumeration, selection by USB UID or
serial, explicit video profiles, depth/RGB/IR capture, frame bytes, timestamps,
depth scale, and checked depth samples. SDK objects release their native
references on Drop. Device and context parents stay alive with their children.

## Setup

Install a Rust toolchain, Python 3, a C compiler, and Clang development files.
On Ubuntu, the normal system dependency setup is:

```bash
sudo apt install build-essential clang libclang-dev python3 pkg-config
source "$HOME/.cargo/env"
python3 scripts/fetch-sdk.py
sudo bash scripts/install-udev-rules.sh
```

The downloader fetches official release archives for both architectures, verifies
their pinned SHA256 hashes, and preserves the libraries' symbolic links. SDK
packages stay in `.sdk/`; binaries are not committed. USB rules give members of
`plugdev` access to Orbbec interfaces; reconnect the camera if required. This PC
already has the rule installed.

`scripts/with-sdk.sh` adds the native SDK library directory to `LD_LIBRARY_PATH`
and adds `~/.cargo/bin` to PATH for the command it runs. It also finds the local
Clang headers prepared for this PC. It does not edit shell profiles.

## Run

Run all camera commands sequentially; one camera cannot be opened by competing
processes.

```bash
# Independent C API baseline; no OpenCV or GUI required.
scripts/c-smoke.sh

# Device information and indexed depth/color/IR profiles.
scripts/with-sdk.sh cargo run -p orbbec-v1 --example enumerate

# Thirty depth frames, three start/stop cycles, then a raw 16-bit PGM.
scripts/with-sdk.sh cargo run -p orbbec-v1 --example capture -- \
  --frames 30 --cycles 3 --output captures/depth.pgm

# RGB + depth; complete framesets do not imply synchronized exposures.
scripts/with-sdk.sh cargo run -p orbbec-v1 --example capture -- --color

# Optional IR, profile index, and serial selection.
scripts/with-sdk.sh cargo run -p orbbec-v1 --example capture -- \
  --ir --depth-profile 0 --serial YOUR_SERIAL
```

Run `capture --help` through Cargo's `--` argument separator for all options.
The output PGM stores raw depth samples in big-endian PGM order; multiply samples
by the SDK's reported `scale_mm` to obtain millimeters. Zero means invalid depth.
The PGM is not a contrast-adjusted preview image.

## Use from your project

```toml
[dependencies]
orbbec-v1 = { path = "/home/gmmyung/orbbec-v1-rs/crates/orbbec-v1" }
```

```rust,no_run
use orbbec_v1::{Context, Pipeline, Sensor};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::new()?;
    let devices = context.devices()?;
    let device = context.open(devices.first().ok_or("No camera")?)?;
    let mut pipeline = Pipeline::new(&device)?;
    let profiles = pipeline.profiles(Sensor::Depth)?;
    let profile = profiles.first().ok_or("No depth profile")?;
    pipeline.start(&[*profile])?;
    {
        if let Some(set) = pipeline.wait(Duration::from_secs(2))? {
            if let Some(depth) = set.depth()? {
                let width = depth.video().width()?;
                let height = depth.video().height()?;
                println!("Center: {:?} mm", depth.distance_mm(width / 2, height / 2)?);
            }
        }
    }
    pipeline.stop()?;
    Ok(())
}
```

When building from another repository, the sys crate still finds this checkout's
`.sdk/` directory. Use this checkout's `scripts/with-sdk.sh cargo run ...`, or
configure your application's runtime library path yourself.

For a separately installed SDK, set `ORBBEC_V1_SDK_DIR` to the directory containing
`include/` and `lib/`. The build checks the library's ELF architecture. The safe
wrapper checks the exact loaded SDK version before constructing a context.
Headers and binaries must come from the same v1.10.37 package. Do not mix SDK v1
and v2 libraries in the same application process.

## ARM64

For native development on ARM64, use the same setup and run commands. The script
selects the ARM64 package automatically. Actual camera capture on ARM64 still
needs validation on the target board.

For cross compilation from x86_64 Ubuntu:

```bash
sudo apt install gcc-aarch64-linux-gnu libc6-dev-arm64-cross
rustup target add aarch64-unknown-linux-gnu
scripts/build-arm64.sh
```

This produces `target/aarch64-unknown-linux-gnu/debug/examples/{enumerate,capture}`.
`ORBBEC_V1_SYSROOT` can select a different target C sysroot. The cross build script
also supports the user-local Ubuntu toolchain extracted into `.tools/` on this
PC. Build against a sysroot compatible with the oldest glibc version you deploy
to; a successful cross build does not establish runtime compatibility with every
Jetson or other ARM64 distribution.

Deploy the target binary with the **ARM64** SDK's `lib/` contents, preserving
symbolic links, and retain its configuration and license files. Set
`LD_LIBRARY_PATH` on the board or configure the system loader. `liblive555.so`
and `libob_usb.so` are runtime dependencies too; copying only `libOrbbecSDK.so`
is insufficient. Do not copy the x86_64 package to the ARM64 board.

## Verification and safety

```bash
scripts/with-sdk.sh cargo test --workspace
scripts/with-sdk.sh cargo clippy --workspace --all-targets -- -D warnings
scripts/with-sdk.sh cargo fmt --all -- --check
# Exclusive access to one physical camera required:
scripts/with-sdk.sh cargo test -p orbbec-v1 --test hardware -- \
  --ignored --test-threads=1
```

Binding generation uses target-specific Clang types and emits compile-time
layout assertions. Tests check depth buffer validation, loaded version, thread
confinement, and compile failures for using frame bytes after release or stopping
with live frames. The hardware test exercises native error conversion, five
start/stop cycles, parent lifetime retention, and automatic stop/reopen on Drop.

The wrapper exposes no mutable SDK buffer views and implements neither `Send`
nor `Sync` for handles. Frames borrow their owners. Keep capture on one dedicated
thread; copy samples or bytes into owned Rust buffers when sending to other
threads. `Pipeline::stop()` reports cleanup errors; Drop attempts cleanup and
frees error objects without panicking. `wait()` distinguishes a null frameset
(`Ok(None)`) from an SDK error (`Err`).

## Stereo S U3 details

The connected camera advertises Y11/Y12 **stream profiles**. With the SDK's
default unpacking, captured depth frames report Y16 and contain two bytes per
pixel. The wrapper checks both the returned format and byte count before
decoding. It does not assume that a Y16 stream profile exists.

The SDK warns that this camera does not support frame synchronization. RGB and
depth SDK timestamps were observed in different clock domains, including an
initial zero RGB timestamp. Do not subtract them to infer exposure skew.
`system_timestamp_us()` exposes the SDK's host reception timestamp separately;
host reception timing is not proof of synchronized exposures.

The SDK emits OpenNI capability warnings and USB cancellation warnings during
shutdown on this PC even after successful capture and stop. See validation notes.

SDK license and redistribution details: [NOTICE.md](NOTICE.md).
