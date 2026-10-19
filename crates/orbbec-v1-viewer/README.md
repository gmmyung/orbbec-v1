# orbbec-v1-viewer

Native egui viewer for legacy Orbbec cameras using SDK 1.10.37. Supports Linux GNU
x86_64 and AArch64 and requires Rust 1.88+ plus an OpenGL Wayland or X11 desktop.

From the workspace:

```bash
cargo run -p orbbec-v1-viewer -- --view cloud
```

Views: `depth`, `rgb`, `ir`, and `cloud`. Select a camera with `--serial SERIAL`.
The point cloud uses calibrated XYZ coordinates. Drag to orbit, scroll to zoom,
right-drag to pan, and press F to fit the cloud. Near/far clips depth; point size,
pixel step, and freeze controls adjust rendering.

The matching SDK libraries are supplied by Cargo dependencies. Camera USB
permissions are still required. `cargo run` configures library loading.
For a standalone executable, place the SDK at `orbbec-sdk/{lib,config}` next to
the executable; this viewer includes an `$ORIGIN/orbbec-sdk/lib` runtime path.
Copy accompanying notices and source materials when preparing a distribution.
The repository's `scripts/package-viewer.py` prepares that layout.

Rust code is MIT licensed; the bundled SDK retains its separate original terms.
Releases use the repository's manual GitHub Actions publishing workflow.

## ARM64 validation

Use the complete bundle for standalone deployment: `cargo install` alone does
not install the SDK's shared libraries next to the executable. Native Ubuntu
24.04 ARM64 bundles are produced by the release verification workflow. A binary
cross-linked on a newer desktop may require a newer glibc than the target has;
`scripts/check-glibc.py` checks this before deployment.

The point renderer has been exercised on NVIDIA Tegra Orin via a headless EGL
pbuffer using real calibrated camera points, shader compilation, GPU draw, and
pixel readback. The ignored `headless_egl_renders_live_point_cloud` test can run
without X11/Wayland. Set `ORBBEC_TEST_GL_VENDOR=NVIDIA` to require that vendor and
`ORBBEC_TEST_CLOUD_PNG` to an absolute output path to save the rendered image.
