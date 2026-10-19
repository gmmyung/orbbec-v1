# Validation on 2026-10-19

## Environment

- Host: x86_64, Ubuntu 26.04 LTS, Linux 7.0.0-34-generic.
- Rust/cargo: 1.99.1; bindgen 0.72.1; libclang 21.
- Camera: Astra Stereo S U3, SDK name `SV1301S_U3`, firmware `RD3013`.
- USB interfaces: RGB `2bc5:0511`, depth `2bc5:0614`.
- Current connection: USB 2.0, 480 Mb/s, through an external USB hub.
- SDK: official v1.10.37 x64 and ARM64 release archives, both verified against
  `scripts/sdk-lock.json`.

The SDK and user-local build tools live in ignored directories. Rust was installed
under `~/.cargo` without changing shell startup files. The user installed the
repository's udev rule, granting this account access to both camera interfaces.

## Results

| Check | Result |
| --- | --- |
| Headless C baseline | 30/30 depth frames, explicit stop and cleanup |
| Rust enumeration | One camera; 24 depth, 21 color, 12 IR profiles |
| Rust depth | 3 cycles × 30 frames, 640×400 at 30 fps |
| Rust RGB + depth | 2 cycles × 30 complete framesets, RGB 640×480 at 30 fps |
| Rust IR + depth | 30 complete framesets, IR 640×400 at 30 fps |
| Rust RGB + depth + IR | 30 complete framesets, selected by the opened device serial |
| Hardware integration test | Passed: native error conversion, 5×30 depth frames, automatic stop/reopen, retained device/context parents |
| Unit tests | 2 passed, plus compile-time assertions that handles are neither Send nor Sync |
| Lifetime documentation tests | 2 compile-fail tests passed |
| Generated C layout assertions | Compiled for x86_64 and AArch64 |
| Clippy | Passed with warnings denied |
| Rust formatting | Passed |
| API documentation | Built successfully for both workspace crates |
| ARM64 examples | Built and linked against the actual ARM64 SDK; ELF machine is AArch64 |

Detailed command output is retained locally under `target/validation/`. The
initial enumeration output is also available at `/tmp/orbbec-v1-enumerate.log`.
The depth capture example saved `captures/depth.pgm` (640×400 raw 16-bit depth).
Camera images and verbose vendor logs are ignored by Git.

## Camera behavior observed

Depth profile 0 is 640×400, 30 fps, Y11. The default SDK processing returns a
Y16 frame with 512,000 bytes and scale 1 mm per sample. A representative frame
contained 237,036 nonzero samples out of 256,000. The center sample was 1,783 mm.
These are SDK measurements; physical distance accuracy was not independently
calibrated.

Color 640×480 RGB frames contain 921,600 bytes. IR profile Y10 produces unpacked
Y16 frames containing 512,000 bytes at 640×400. No assumptions about tightly packed
11-bit or 12-bit depth are made when reading samples.

Enumeration reports an empty serial string on this camera. Opening it and reading
device information returns a serial, so the wrapper's `open_by_serial` checks
opened device information instead of relying on the enumeration serial.

The SDK itself warns about unsupported frame synchronization while creating a
pipeline, even though the wrapper does not request synchronization. Captured
depth timestamps resemble host epoch microseconds; color SDK timestamps were
initially zero and later had a different clock origin. Complete framesets must
not be interpreted as proof of synchronized exposures. Host reception timestamps
are available separately through `system_timestamp_us()`.

The vendor SDK prints warnings about unsupported OpenNI capabilities and USB
transfer cancellation during stop. The C baseline, repeated Rust captures, and
subsequent reopen all completed successfully despite those warnings.

## ARM64 and remaining hardware checks

The ARM64 examples are cross compiled with a user-local GCC 15 / Ubuntu 26.04
cross sysroot. The capture executable references glibc symbol versions up to
`GLIBC_2.34`; use a compatible target OS or rebuild against its older sysroot.
Runtime deployment must include ARM64 SDK dependencies, configuration, and
applicable notices.

Physical ARM64 capture, unplug/replug behavior, long-duration memory behavior,
and two-camera RGB/depth pairing have not been tested. The hardware test and C
baseline are ready to run on an ARM64 board. No firmware changes were made.
