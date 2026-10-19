# Jetson AGX Orin validation — 2026-10-19

## Tested system

| Item | Observed value |
| --- | --- |
| Board | NVIDIA Jetson AGX Orin Developer Kit, AArch64 |
| OS | Ubuntu 24.04.5 LTS, glibc 2.39 |
| NVIDIA release | L4T R39.2.1, kernel 6.8.12-1021-tegra |
| Native toolchain | rustc/cargo 1.99.0 |
| Camera | Astra Stereo S U3, SDK name SV1301S_U3, firmware RD3013 |
| SDK | Original bundled ARM64 v1.10.37 libraries |
| GPU renderer | NVIDIA Tegra Orin (nvgpu)/integrated |
| GPU API | OpenGL 3.3.0 NVIDIA 595.78 through EGL device + pbuffer |

The camera exposes USB2.0 RGB and depth interfaces. The existing account initially
had no write access. The repository's udev rule was installed and the account
added to `plugdev`. Tests used `sg plugdev` to activate the new group in SSH
sessions without restarting the machine. Future desktop logins pick up the group
membership normally. No login credentials are stored in the repository.

## Results

| Check | Result |
| --- | --- |
| Native workspace tests | Unit tests and compile-fail documentation tests passed |
| SDK/device enumeration | SDK 1.10.37; one legacy camera opened successfully |
| Depth + RGB, desktop-cross-built CLI | 30 frames × 3 start/stop cycles passed |
| Native depth + IR selected by serial | 30 frames × 2 cycles passed |
| Native capture/error recovery test | Five start/stop cycles, invalid profiles, parent lifetime retention, and reopen passed |
| Native calibrated XYZ test | 640×400 cloud, 231,004 valid points; axes/metric Z checks passed |
| Native viewer camera worker | Depth, RGB, IR; two start/stop/reopen cycles passed |
| Actual NVIDIA GPU point renderer | Shader compilation, draw, readback, and PNG output passed with no GL error |
| Native standalone viewer bundle | Loads SDK and runs `--help` with no loader override |
| Deployment ABI check | Native executable's highest referenced glibc version is 2.39 |

Profiles were depth 640×400 @30 Y11, RGB 640×480 @30 RGB, and infrared
640×400 @30 Y10. Captured depth frames were unpacked Y16. Center depth was around
1.85–1.87 m; around 231,000 of 256,000 pixels were valid in the static scene.
Calibration was fx=fy=477.41806, cx=316.40866, cy=198.3181 at 640×400.
The XYZ test checks conversion consistency, not physical distance accuracy against
an independently measured reference.

The ignored EGL test captures a real SDK point cloud, uses the viewer's own
`PointRenderer`, fits the same orbit camera, draws to a 640×480 depth-buffered
pbuffer, and reads back pixels. With `ORBBEC_TEST_GL_VENDOR=NVIDIA`, software
fallback is rejected. The successful run rendered 57,742 sampled calibrated
points into 31,663 colored pixels. The PNG was copied back and inspected at
`target/validation/jetson-headless-cloud.png`.

The system was at the GDM login screen, so a full interactive egui desktop window
was not exercised. The camera worker and actual GPU point rendering were tested
independently of a window server. The SDK emitted known unsupported frame-sync
messages and USB cancellation warnings during stop despite successful frame
capture, clean test completion, and reopen. These messages were retained in logs.

## Portability finding and changes

The viewer cross-linked on the Ubuntu 26.04 development PC references
`atan2f@GLIBC_2.43` and `acosf@GLIBC_2.43`. That binary cannot run on this glibc
2.39 target. The cross-built CLI only references glibc versions through 2.34 and
was successfully exercised on the Orin. Building the viewer natively on the Orin
produced a compatible executable (highest referenced glibc version 2.39).

`scripts/check-glibc.py` now rejects ELF files that exceed a supplied target
ceiling. It rejected the desktop-built viewer for a 2.39 ceiling and accepted
the native Orin build. This is a conservative symbol-version check, not a complete
audit of all transitive libraries, drivers, kernels, or other Jetson releases.

The manual publish workflow now gates publication on native ARM64 Ubuntu 24.04
tests as well as the existing x86_64 verification. It produces a complete native
ARM64 viewer bundle and enforces glibc 2.39 on both ARM64 builds. GitHub-hosted
ARM64 runners have no physical camera; their hardware/EGL tests remain ignored.

The first hosted ARM64 run also exposed an SDK crash when libusb initialization
fails in an isolated USB environment. The safe wrapper now probes the same bundled
USB backend using a temporary context before entering SDK context creation,
returning a Rust error if it fails. A conditional unit test exercises this failure
path on the hosted runner; device enumeration stays in the physical hardware tests.

## Reproduce on a configured Orin

After logging in with the camera's USB permissions active:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --workspace --locked
cargo test -p orbbec-v1 --test hardware --locked -- --ignored --test-threads=1
cargo test -p orbbec-v1-viewer --locked worker_captures_owned_frames \
  -- --ignored --nocapture --test-threads=1
mkdir -p target/validation
ORBBEC_TEST_GL_VENDOR=NVIDIA \
ORBBEC_TEST_CLOUD_PNG="$PWD/target/validation/jetson-headless-cloud.png" \
cargo test -p orbbec-v1-viewer --locked headless_egl_renders_live_point_cloud \
  -- --ignored --nocapture --test-threads=1
python3 scripts/package-viewer.py
python3 scripts/check-glibc.py target/debug/orbbec-v1-viewer --max-version 2.39
```

On the target, the native standalone viewer is left at
`~/orbbec-v1/target/dist/orbbec-v1-viewer-aarch64/orbbec-v1-viewer`.
After a desktop login, launch it with `--view cloud`.

Detailed logs were retained in the target's `~/orbbec-v1/target/validation/`
and copied to the development PC's `target/validation/jetson/`.

## Published crates.io 0.1.0 validation — 2026-10-19

After publication, a separate project was created on the same Orin at
`~/orbbec-v1-cratesio-test.2ID3pc/`. Its dependency is:

```toml
[dependencies]
orbbec-v1 = "=0.1.0"
```

A fresh `CARGO_HOME` inside that directory prevented reuse of the development
checkout's registry cache. SDK, Clang, and loader overrides were removed. Cargo
metadata confirmed that the API, sys, and AArch64 binary dependencies all came
from crates.io, with no path patches and no x86_64 binary dependency. Cached
archive checksums matched their Cargo lockfile entries, and all native payload hashes
matched the packaged provenance.

The published API and viewer archives were also downloaded directly from
crates.io and verified against registry checksums. The API's hardware tests and
examples ran in the fresh consumer. The viewer was built and tested from its
published archive, using the registry API/sys/native dependencies.

| Check | Result on the requested retry |
| --- | --- |
| Fresh registry consumer | Built and loaded SDK 1.10.37 |
| Serial selection, depth + RGB | 300 frames × 3 start/stop cycles passed |
| Serial selection, depth + IR | 150 frames × 2 start/stop cycles passed |
| Sustained depth with RGB | All 120 frames after 30-frame warmup were nonempty; minimum 238,336 valid pixels out of 256,000 |
| Sustained depth with IR | All 120 frames after 30-frame warmup were nonempty; minimum 238,688 valid pixels out of 256,000 |
| Published API hardware tests | Both calibrated XYZ and repeated capture/error recovery passed |
| Calibrated XYZ | 640×400, 239,716 valid points; metric depth and axes checks passed |
| Published viewer | Five unit tests and both camera/GPU hardware tests passed |
| NVIDIA GPU readback | 59,917 sampled points, 33,226 colored pixels; no GL error |
| Native viewer ABI | Highest referenced glibc version 2.39; `--help` passed |

RGB capture cycles took 11.23–11.40 seconds for 300 frames including startup and
stop; IR cycles took 5.83–5.84 seconds for 150 frames. Final center depth was
about 1.87 m. These are functional checks, not a sustained performance benchmark.

### Initial empty-depth finding

The initial capture runs delivered frames but later depth buffers were entirely
zero. The existing source checkout reproduced the same behavior. A read-only
property probe observed `LDP enabled=true`, `LDP protection active=true`, and
`Laser enabled=false`; a later query observed protection inactive and the laser
enabled. Orbbec documents that laser distance protection can turn off the laser
when an obstacle is too close; see the SDK v1 guide's LDP sections in
[the official API guide](https://orbbec.github.io/OrbbecSDK/doc/tutorial/English/OrbbecSDK_C%2B%2B_API_user_guide-v1.0.pdf).

After the user requested a retry, the capture cycles, hardware tests, and GPU
test were repeated successfully. Additional checks required nonempty depth
throughout the post-warmup samples. Successful frame delivery alone would have
missed this condition; the initial first-frame point-cloud test also passed
before the later frames became empty.

The graphical desktop was still at GDM, so interactive window controls were
not tested. Actual NVIDIA rendering used the published viewer's headless EGL
test. SDK frame-sync and USB cancellation warnings remained in the logs.

The test project, isolated Cargo cache, logs, and rendered PNG remain on the
Jetson. Its `report.json` records dependency paths, archive hashes, and results.
Local copies are under `target/validation/jetson-cratesio/`, including
`after/registry-point-cloud.png` and the complete before/after logs.
