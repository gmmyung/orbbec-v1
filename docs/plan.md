# Implementation status

Reference camera: Astra Stereo S U3, SDK device name SV1301S_U3, RD3013 firmware.
Targets: x86_64-unknown-linux-gnu and aarch64-unknown-linux-gnu.

- [x] Initialize a local repository and pin official v1.10.37 packages by SHA256.
- [x] Obtain USB permissions and inspect both RGB and depth USB interfaces.
- [x] Compile and run a headless C capture baseline on x86_64.
- [x] Generate raw C bindings with bindgen for both target architectures.
- [x] Implement the safe synchronous core and examples.
- [x] Capture depth, RGB + depth, IR + depth, and RGB + depth + IR on the reference camera.
- [x] Test error handling, repeated start/stop, and automatic cleanup/reopen.
- [x] Build and link ARM64 examples against the actual ARM64 SDK.
- [ ] Run the same C baseline and hardware test on a physical ARM64 board.
- [ ] Test unplug/replug recovery on hardware.
- [ ] Verify RGB/depth physical pairing with two cameras if needed by the project.

Extend the safe wrapper as application requirements emerge: camera calibration,
alignment, filters, camera properties, then recording/playback or callbacks.
The corresponding SDK C calls are already available in the raw sys crate.
Callbacks require a separate ownership and shutdown design before adding a safe
API. Preserve thread confinement unless SDK thread safety has been established.
