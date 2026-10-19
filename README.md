# orbbec-v1

Rust bindings for **Orbbec SDK v1.10.37**, initially developed with an **Astra
Stereo S U3** (`SV1301S_U3`, firmware `RD3013`). Targets are Linux GNU x86_64 and
AArch64. This is a local development repository; hardware results are recorded
in [docs/validation.md](docs/validation.md).

The workspace contains architecture-specific binary packages, `orbbec-v1-sys`
(pregenerated C FFI) and `orbbec-v1` (a safe,
synchronous wrapper), plus `orbbec-v1-viewer` (a native egui viewer). The safe API covers enumeration, selection by USB UID or
serial, explicit video profiles, depth/RGB/IR capture, frame bytes, timestamps,
depth scale, and checked depth samples. SDK objects release their native
references on Drop. Device and context parents stay alive with their children.

## Setup

Install a Rust toolchain and a native C linker. The matching SDK libraries are
bundled in architecture-specific Cargo dependencies. Building Rust crates needs
no separate SDK download, Python, or libclang. On Ubuntu:

```bash
sudo apt install build-essential pkg-config
source "$HOME/.cargo/env"
sudo bash scripts/install-udev-rules.sh
cargo build --workspace
```

USB rules give members of `plugdev` access to Orbbec interfaces; reconnect the
camera if required. This PC already has the rule installed. The egui viewer also
needs a working Wayland or X11 desktop and OpenGL drivers.

`cargo run` and `cargo test` configure the runtime library path automatically.
`scripts/with-sdk.sh` remains a convenience for adding `~/.cargo/bin` to PATH;
it supports an explicitly selected `ORBBEC_V1_SDK_DIR` and the local Clang headers
used only by the binding-regeneration tool. It does not edit shell profiles.

Original SDK ZIPs remain cached in the ignored `.sdk/` directory for maintainer
work. The minimal native libraries, configuration, license texts, provenance,
and upstream companion-library source archives are now included under
`crates/orbbec-v1-bin-{x86_64,aarch64}/` for packaging. See
[distribution details and source status](docs/distribution.md).

## Run

For the native egui viewer (Rust 1.88+, Wayland or X11 desktop with OpenGL):

```bash
scripts/viewer.sh
# Optional: start in another view or select a specific camera.
scripts/viewer.sh --view rgb
scripts/viewer.sh --serial YOUR_SERIAL
# Open the interactive 3D point cloud directly:
scripts/viewer.sh --view cloud
```

The viewer starts capture automatically. Switch between **Depth**, **RGB**, and
**Infrared** using the tabs. Adjust **Near/Far** to change depth colors; invalid
pixels stay black. Hover over depth pixels to read their distance in millimeters.
**Stop** releases the camera; **Start** reconnects it. Closing the window stops
capture and releases SDK handles. The FPS, center distance, and active profiles
are shown below the image.

The **Point cloud** tab uses the SDK's calibrated XYZ filter and renders points
with OpenGL depth testing. Drag to orbit, scroll to zoom, and right-drag or
Shift-drag to pan. Press **F**, double-click, or use **Fit view** to reframe the
cloud. **Near/Far** clips depth in meters; **Point size** adjusts dot size and
**Pixel step** trades density for rendering speed. **Freeze cloud** retains the
current 3D frame while you inspect it. Points are colored by depth.

XYZ conversion runs on the capture thread only when the point cloud tab is
selected. No camera field of view is guessed: calibration comes from the active
pipeline. Zero depth samples are excluded, so a camera aimed too close to an
object may produce a sparse or empty cloud. Try a scene about 0.5–3 meters away
and use **Fit view** after moving the camera.

Capture runs on one background thread. The GUI receives owned buffers through
one replaceable slot so slow painting cannot build up a queue of old frames.
RGB/IR tabs are disabled when their streams are unavailable. Complete framesets
do not imply synchronized exposures.

Run all camera commands sequentially; one camera cannot be opened by competing
processes.

```bash
# Optional maintainer C API baseline; requires original SDK headers.
python3 scripts/fetch-sdk.py
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

When building from another repository, Cargo obtains the matching SDK from the
binary crate dependency. `cargo run` and `cargo test` load it automatically.
For standalone deployment, copy `orbbec_v1::sdk_directory()` and configure the
runtime loader path. The prepared crates are local and have not been published;
a registry dependency will become usable after publication.

For a separately installed SDK, set `ORBBEC_V1_SDK_DIR` to the directory containing
`lib/` and optionally `config/`. The build checks every runtime library's ELF architecture. The safe
wrapper checks the exact loaded SDK version before constructing a context.
Pregenerated bindings and binaries must match v1.10.37. Do not mix SDK v1
and v2 libraries in the same application process.

## ARM64

For native development on ARM64, use the same setup and run commands. Cargo
selects the ARM64 binary dependency automatically. Actual camera capture on ARM64 still
needs validation on the target board.

For cross compilation from x86_64 Ubuntu:

```bash
sudo apt install gcc-aarch64-linux-gnu libc6-dev-arm64-cross
rustup target add aarch64-unknown-linux-gnu
scripts/build-arm64.sh
# Build the egui viewer for an ARM64 desktop:
scripts/build-arm64.sh --viewer
```

This produces `target/aarch64-unknown-linux-gnu/debug/examples/{enumerate,capture}`.
The `--viewer` option produces `target/aarch64-unknown-linux-gnu/debug/orbbec-v1-viewer`.
`ORBBEC_V1_SYSROOT` can select a different target C sysroot for maintainer binding
regeneration; normal Cargo builds use pregenerated bindings. The cross build script
also supports the user-local Ubuntu toolchain extracted into `.tools/` on this
PC. Build against a sysroot compatible with the oldest glibc version you deploy
to; a successful cross build does not establish runtime compatibility with every
Jetson or other ARM64 distribution.

Deploy the target binary with the **ARM64** SDK's staged `lib/` contents and
retain its configuration, license, provenance, and applicable source materials. Set
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
# Viewer worker capture/stop/reopen on the reference camera:
scripts/with-sdk.sh cargo test -p orbbec-v1-viewer -- --ignored --test-threads=1
# Open the viewer, capture 30 frames, save its window, then exit:
scripts/viewer.sh --smoke-test target/validation/viewer-depth.png
```

Pregenerated bindings use target-specific Clang types and contain compile-time
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

## Calibrated XYZ in your application

The safe wrapper exposes `PointCloudFilter` and owned `PointCloud` buffers:

```rust
use orbbec_v1::{Pipeline, PointCloudFilter, Result};

// Call after starting/configuring the pipeline.
fn capture_cloud(pipeline: &Pipeline) -> Result<()> {
    let mut filter = PointCloudFilter::new(pipeline)?;
    if let Some(frames) = pipeline.wait(std::time::Duration::from_secs(1))? {
        if let Some(cloud) = filter.process(&frames)? {
            // One XYZ triplet per depth pixel, in meters. Invalid points are zero.
            // Camera coordinates: X right, Y down, Z forward.
            let points: Vec<[f32; 3]> = cloud.points;
            println!("{} × {} points, {} valid", cloud.width, cloud.height,
                points.iter().filter(|p| p[2] > 0.0).count());
        }
    }
    Ok(())
} // Filter is dropped here; the caller can now stop/reconfigure the pipeline.
```

`PointCloud` contains owned Rust data and is `Send + Sync`; the filter remains on
its capture thread. The viewer rotates camera axes to Y-up/-Z-forward for its
OpenGL display, while library output stays in the SDK's camera frame.

SDK v1.10.37 declares `ob_pointcloud_filter_set_coordinate_system` in its C header
but does not export that C symbol from its Linux library. The safe wrapper uses
the native filter's default camera axes and performs display transforms in Rust.

The SDK emits OpenNI capability warnings and USB cancellation warnings during
shutdown on this PC even after successful capture and stop. See validation notes.

SDK license and redistribution details: [NOTICE.md](NOTICE.md).

## Publishing

The manual [GitHub publish workflow](.github/workflows/publish.yml) verifies by
default and uploads only when explicitly requested. All packages use the
`orbbec-v1` prefix. See [publishing setup](docs/publishing.md) for the first-release
secret and subsequent trusted publishing configuration.
