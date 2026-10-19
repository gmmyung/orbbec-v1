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

## egui viewer

Added `orbbec-viewer` with eframe/egui 0.33.3 and the Glow renderer. The viewer
requires Rust 1.88 or newer and a Wayland/X11 desktop with OpenGL support.
On this PC, each of the depth, RGB, and infrared window smoke tests successfully
captured at least 30 framesets, saved the application's framebuffer, and exited
after stopping the camera. Previews showed approximately 30 capture FPS.

- `target/validation/viewer-depth.png`: depth colors, range sliders, and center distance.
- `target/validation/viewer-rgb.png`: RGB preview at 640×480.
- `target/validation/viewer-ir.png`: unpacked, normalized infrared preview at 640×400.

All three saved windows were inspected. The worker hardware test passed two
capture/stop/reopen cycles using owned depth, RGB, and IR buffers. Three new unit
tests passed for depth coloring, IR normalization, and IR format/byte validation.
The existing wrapper tests and lifetime documentation tests still pass. Clippy
passed for all workspace targets with warnings denied.

`scripts/build-arm64.sh --viewer` successfully built and linked the native viewer
against the ARM64 SDK. A physical ARM64 desktop has not been tested. Logs for the
viewer build, Clippy, unit tests, worker hardware test, and ARM64 build are kept
under `target/validation/viewer-*.log`.

## Point cloud visualization

Added a safe `PointCloudFilter` using the SDK's native XYZ filter and active
pipeline calibration. On the connected Stereo S U3, calibrated depth intrinsics
were fx=fy=477.41806, cx=316.40866, cy=198.3181 at 640×400. The hardware test
received 31,771 valid points in one startup frame, checked X/Y axis signs against
pixel positions, and confirmed Z agrees with the depth samples in meters within
2 mm numerical tolerance. This checks conversion and units, not physical depth
accuracy against an independently measured scene.

The library returns owned, organized XYZ buffers in meters, X right/Y down/Z
forward. The renderer rotates Y/Z for OpenGL's Y-up/-Z-forward view. Only valid
points in the selected Near/Far range are uploaded. GPU depth testing handles
occlusion. Orbit, pan, zoom, fit, freeze, point size, and pixel sampling density
controls are available in the Point cloud tab. XYZ conversion runs only while
that tab is selected. Frozen clouds retain their own immutable owned frame.

The native window smoke test rendered the live cloud and saved
`target/validation/viewer-point-cloud.png`. The screenshot was inspected. Capture
was about 30 FPS; the then-current close-up desk scene yielded mostly invalid
depth and only a small number of points in the default 0.3–4 m display range.
A separate depth-only capture confirmed this scene behavior without the point
cloud filter: its last frame had zero valid depth samples. The RGB reference
window (`point-cloud-rgb-reference.png`) showed that the camera had been moved
from the earlier ceiling view to the nearby desk. No synthetic points are
substituted for missing depth.

The SDK v1.10.37 C header declares
`ob_pointcloud_filter_set_coordinate_system`, but its x64 Linux library does not
export it. The wrapper uses the filter's default axes; display transforms happen
in Rust. This avoids linking against the unavailable setter.

Unit tests cover XYZ byte decoding, scale, invalid values, range clipping,
coordinate rotation, orbit projection, and zoom limits. Compile-time checks
ensure `PointCloud` is Send+Sync and the SDK filter stays confined to its capture
thread. A documentation test verifies a live filter prevents stopping its
pipeline. Workspace tests and Clippy pass. The ARM64 viewer builds and links;
physical ARM64 capture remains untested. Logs are in
`target/validation/point-cloud-*.log` and `viewer-point-cloud.log`.

## Bundled crate packaging (2026-10-19)

Added separate x86_64 and AArch64 binary crates with original SDK libraries,
configuration, complete SDK notice, additional upstream license texts, upstream
source snapshots, and per-file SHA-256 provenance. `vendor-sdk.py` compares each
library with its entry in the checksum-verified official ZIP. Library bytes are
unchanged. Pregenerated Rust bindings replace consumer-side bindgen; regeneration
with the separate maintainer tool reproduced both checked-in target files.

`cargo package --workspace --allow-dirty` verified all five packages. Binary crate
archive sizes were 6,269,865 bytes for x86_64 and 6,054,736 bytes for AArch64, below
the conservative 10,000,000-byte check. The distribution verification script
checked packaged payload hashes and extracted the normalized published manifests
into `/tmp`, then ran the safe wrapper's tests offline with all SDK, Clang, and
loader environment overrides removed. All three unit tests and three compile-fail
documentation tests passed. Target-specific dependency trees selected only the
matching binary crate. The isolated run uses no source files or SDK from the
checkout. Its target artifacts are under `target/distribution-check/`.

Workspace tests (8 unit and 3 compile-fail documentation tests), Clippy with
warnings denied, formatting, and rustdoc with warnings denied passed. The ARM64
viewer linked successfully. Native workspace tests and an ARM64 sys-crate/viewer
rebuild also passed while the maintainer `.sdk/` directory was temporarily moved
out of its usual path. The cross-linker now uses the vendored ARM64 package for
dependent-library resolution, rather than the ignored SDK cache.

`package-viewer.py` built `target/dist/orbbec-viewer-x86_64/` with an executable
RUNPATH of `$ORIGIN/orbbec-sdk/lib`. A complete copy relocated under `/tmp` started
the GUI and SDK without loader environment overrides and while `.sdk/` was absent.
Its camera worker returned "No camera found". `lsusb` confirmed neither Orbbec USB
interface was connected during this validation. The live screenshot smoke test
therefore timed out and did not produce a screenshot; fresh camera capture was
not validated in this packaging change. The log is
`target/validation/bundled-viewer-smoke.log`. The maintainer cache was restored.

No packages were uploaded. The upstream live555 version/license discrepancy and
unverified correspondence of upstream snapshots to Orbbec's private builds are
recorded in `docs/distribution.md` and each binary crate's `THIRD_PARTY.md` and
`provenance.json`. Included snapshots are not a declaration of full LGPL compliance.

## Jetson hardware and GPU validation (2026-10-19)

Native AArch64 capture, calibrated XYZ conversion, and the actual NVIDIA Tegra
Orin GPU point renderer have now been validated on a Jetson AGX Orin Developer
Kit with Ubuntu 24.04, glibc 2.39, and the reference Astra Stereo S U3. All native
workspace, camera recovery, serial-selected IR capture, and viewer worker tests
passed. A new ignored EGL test rendered 57,742 real calibrated points on NVIDIA
hardware without a desktop login. A native viewer bundle loaded successfully.
See [the complete test record](jetson-validation.md), including the desktop cross
build's incompatible glibc 2.43 requirement, native build results, and remaining
desktop-interaction boundary. Native ARM64 CI and a glibc compatibility gate were
added to the manual release workflow.
