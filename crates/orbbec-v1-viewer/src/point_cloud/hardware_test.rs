//! Exercise the actual point renderer and a real camera without a window server.
use super::*;
use glutin::{
    api::egl::{device::Device, display::Display},
    config::{Api, ConfigSurfaceTypes, ConfigTemplateBuilder},
    context::{ContextApi, ContextAttributesBuilder, Version},
    prelude::*,
    surface::{PbufferSurface, SurfaceAttributesBuilder},
};
use orbbec_v1::{Context, Pipeline, PointCloudFilter, Sensor};
use std::{ffi::CString, num::NonZeroU32, time::Duration};

#[test]
#[ignore = "requires an EGL GPU device and exclusive access to one connected Orbbec camera"]
fn headless_egl_renders_live_point_cloud() -> Result<(), Box<dyn std::error::Error>> {
    let display = Device::query_devices()?
        // SAFETY: No raw display handle or borrowed window-system resources.
        .find_map(|device| unsafe { Display::with_device(&device, None) }.ok())
        .ok_or("No usable EGL device display")?;
    let template = ConfigTemplateBuilder::new()
        .with_api(Api::OPENGL)
        .with_surface_type(ConfigSurfaceTypes::PBUFFER)
        .with_depth_size(24)
        .build();
    // SAFETY: This config uses only owned EGL device resources, no raw handles.
    let config = unsafe { display.find_configs(template)? }
        .next()
        .ok_or("No OpenGL pbuffer configuration with a depth buffer")?;
    let width = 640_u32;
    let height = 480_u32;
    let surface_attributes = SurfaceAttributesBuilder::<PbufferSurface>::new().build(
        NonZeroU32::new(width).unwrap(),
        NonZeroU32::new(height).unwrap(),
    );
    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(None);
    // SAFETY: Config, surface, and context share this display and stay alive
    // throughout all GL work. No window is used or required.
    let surface = unsafe { display.create_pbuffer_surface(&config, &surface_attributes)? };
    let context = unsafe { display.create_context(&config, &context_attributes)? };
    let _current_context = context.make_current(&surface)?;
    // SAFETY: The context is current on this thread; EGL provides its entry points.
    let gl = unsafe {
        glow::Context::from_loader_function(|name| {
            display.get_proc_address(&CString::new(name).unwrap())
        })
    };
    // SAFETY: GL string queries require only the current context.
    let (vendor, renderer, version) = unsafe {
        (
            gl.get_parameter_string(glow::VENDOR),
            gl.get_parameter_string(glow::RENDERER),
            gl.get_parameter_string(glow::VERSION),
        )
    };
    eprintln!("EGL vendor={vendor}; renderer={renderer}; version={version}");
    if let Ok(expected) = std::env::var("ORBBEC_TEST_GL_VENDOR") {
        assert!(vendor.contains(&expected), "Expected GL vendor {expected}");
    }

    let camera = Context::new()?;
    let devices = camera.devices()?;
    assert_eq!(devices.len(), 1, "Connect exactly one camera for this test");
    let device = camera.open(&devices[0])?;
    let mut pipeline = Pipeline::new(&device)?;
    let profiles = pipeline.profiles(Sensor::Depth)?;
    pipeline.start(&[profiles[0]])?;
    let cloud = {
        let mut filter = PointCloudFilter::new(&pipeline)?;
        let mut result = None;
        for _ in 0..60 {
            let Some(frames) = pipeline.wait(Duration::from_secs(1))? else {
                continue;
            };
            let Some(cloud) = filter.process(&frames)? else {
                continue;
            };
            if vertices(&cloud, 2, 0.1, 5.0).0.len() > 100 {
                result = Some(cloud);
                break;
            }
        }
        result.ok_or("No renderable calibrated cloud in 60 frames")?
    };
    pipeline.stop()?;
    let (points, bounds) = vertices(&cloud, 2, 0.1, 5.0);
    let count = points.len();
    let mut orbit = Orbit::default();
    orbit.fit(bounds.ok_or("No point cloud bounds")?);
    let mut gpu = PointRenderer::new(&gl)?;
    gpu.pending = Some(points);
    // SAFETY: The pbuffer is current, dimensions match its allocation, and the
    // readback buffer has exactly four bytes per pixel. No borrowed GL pointers.
    let pixels = unsafe {
        gl.viewport(0, 0, width as i32, height as i32);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gpu.paint(
            &gl,
            &orbit.matrix(width as f32 / height as f32).to_cols_array(),
            3.0,
            0.1,
            5.0,
        );
        gl.finish();
        let mut pixels = vec![0_u8; (width * height * 4) as usize];
        gl.read_pixels(
            0,
            0,
            width as i32,
            height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut pixels)),
        );
        assert_eq!(gl.get_error(), glow::NO_ERROR, "Point renderer GL error");
        pixels
    };
    gpu.destroy(&gl);
    let colored = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[..3].iter().any(|&v| v > 10))
        .count();
    assert!(
        colored > 100,
        "GPU readback did not contain a visible cloud"
    );
    eprintln!("Rendered {count} real calibrated points into {colored} colored pixels");
    if let Some(path) = std::env::var_os("ORBBEC_TEST_CLOUD_PNG") {
        let path = std::path::PathBuf::from(path);
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut image = image::RgbaImage::from_raw(width, height, pixels).unwrap();
        image::imageops::flip_vertical_in_place(&mut image);
        image.save(path)?;
    }
    Ok(())
}
