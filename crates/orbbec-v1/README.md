# orbbec-v1

Safe, synchronous Rust capture for legacy Orbbec cameras using **SDK 1.10.37**.
Developed with the Astra Stereo S U3 (`SV1301S_U3`). Supports Linux GNU x86_64 and
AArch64; actual camera capture and calibrated XYZ conversion have been validated
on x86_64 and a Jetson AGX Orin running Ubuntu 24.04.

```toml
[dependencies]
orbbec-v1 = "0.1.0"
```

The dependency supplies the matching precompiled SDK and pregenerated bindings.
No separate SDK installation or libclang is required. Linux USB permissions must
allow access to the camera; install Orbbec's udev rules and reconnect the device.

```rust,no_run
use orbbec_v1::{Context, Pipeline, Sensor};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::new()?;
    let devices = context.devices()?;
    let device = context.open(devices.first().ok_or("No camera")?)?;
    let mut pipeline = Pipeline::new(&device)?;
    let profiles = pipeline.profiles(Sensor::Depth)?;
    pipeline.start(&[profiles[0]])?;
    {
        if let Some(frames) = pipeline.wait(Duration::from_secs(2))? {
            if let Some(depth) = frames.depth()? {
                println!("Depth scale: {} mm", depth.scale_mm()?);
            }
        }
    }
    pipeline.stop()?;
    Ok(())
}
```

Includes enumeration, explicit profiles, depth/RGB/IR capture, borrowed buffers,
timestamps, depth scale, and calibrated XYZ point clouds. SDK handles stay on
one thread; frames borrow their owner. Copy data before passing it to another
thread.

## Runtime deployment

`cargo run` and `cargo test` find the bundled libraries automatically. For a
standalone executable, copy `sdk_directory()` (containing `lib/` and `config/`)
to your application distribution, retain the SDK notices and applicable source
materials, and set `LD_LIBRARY_PATH` to its `lib/` or add an executable RPATH.
Setting `ORBBEC_V1_SDK_DIR` at build time selects an existing SDK installation.
The SDK remains dynamically linked and requires compatible Linux system libraries.

The Rust wrapper is MIT licensed. SDK binaries have separate Orbbec and
third-party terms; they are not relicensed as MIT. The binary dependency records
source provenance and remaining vendor-source verification for public releases.

Release automation and setup: https://github.com/gmmyung/orbbec-v1/blob/main/docs/publishing.md
