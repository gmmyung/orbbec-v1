use eframe::egui;
use orbbec_v1::{Context, Format, Pipeline, Sensor, VideoFrame, VideoProfile};
use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type CaptureResult<T> = Result<T, Box<dyn Error>>;

pub struct Plane<T> {
    pub size: [usize; 2],
    pub pixels: Vec<T>,
}

/// Only owned Rust buffers cross the thread boundary; all SDK objects stay in run().
pub struct Capture {
    pub depth: Plane<u16>,
    pub scale_mm: f32,
    pub color: Option<Plane<u8>>,
    pub infrared: Option<Plane<u16>>,
    pub frame_index: u64,
    pub count: u64,
    pub fps: f32,
}

pub struct CameraState {
    pub message: String,
    pub device: String,
    pub streams: String,
    pub error: bool,
    pub latest: Option<Capture>,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            message: "Connecting…".into(),
            device: String::new(),
            streams: String::new(),
            error: false,
            latest: None,
        }
    }
}

pub struct Worker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(
        ctx: egui::Context,
        state: Arc<Mutex<CameraState>>,
        serial: Option<String>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let thread = thread::spawn(move || {
            let result = run(&ctx, &state, &worker_stop, serial.as_deref());
            let mut state = state.lock().unwrap();
            match result {
                Ok(()) => state.message = "Stopped".into(),
                Err(error) => {
                    state.message = error.to_string();
                    state.error = true;
                    eprintln!("Capture error: {error}");
                }
            }
            ctx.request_repaint();
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            eprintln!("Camera worker panicked");
        }
    }
}

fn run(
    ctx: &egui::Context,
    shared: &Mutex<CameraState>,
    stop: &AtomicBool,
    serial: Option<&str>,
) -> CaptureResult<()> {
    let context = Context::new()?;
    let devices = context.devices()?;
    let device = if let Some(serial) = serial {
        context.open_by_serial(serial)?
    } else {
        if devices.len() > 1 {
            return Err("Multiple cameras connected. Start viewer with --serial SERIAL.".into());
        }
        context.open(devices.first().ok_or(
            "No camera found. Connect the camera, check USB permissions, then press Start.",
        )?)?
    };
    let info = device.info()?;
    let mut pipeline = Pipeline::new(&device)?;
    let depth =
        preferred(&pipeline.profiles(Sensor::Depth)?).ok_or("No depth profiles available")?;
    // RGB output avoids adding a video decoder to the viewer. SDK v1 performs
    // MJPEG-to-RGB conversion for the supported Stereo S U3 RGB profiles.
    let color = pipeline.profiles(Sensor::Color).ok().and_then(|profiles| {
        preferred(
            &profiles
                .into_iter()
                .filter(|p| p.format == Format::RGB)
                .collect::<Vec<_>>(),
        )
    });
    let infrared = pipeline
        .profiles(Sensor::Infrared)
        .ok()
        .and_then(|profiles| preferred(&profiles));
    let mut profiles = vec![depth];
    profiles.extend(color);
    profiles.extend(infrared);
    {
        let mut state = shared.lock().unwrap();
        state.device = format!(
            "{}  ·  {}  ·  firmware {}",
            info.name, info.serial, info.firmware
        );
        state.streams = profiles
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("  |  ");
        state.message = "Starting streams…".into();
    }
    ctx.request_repaint();
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    pipeline.start(&profiles)?;
    eprintln!("Viewer camera: {}", info.name);
    for profile in &profiles {
        eprintln!("Viewer stream: {profile}");
    }
    let mut count = 0_u64;
    let mut fps = 0.0;
    let mut fps_start = Instant::now();
    let mut fps_count = 0;
    let mut last_frame = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let Some(set) = pipeline.wait(Duration::from_millis(100))? else {
            if last_frame.elapsed() > Duration::from_secs(5) {
                return Err("No complete frames received for five seconds. Check the USB connection and press Start to reconnect.".into());
            }
            continue;
        };
        let Some(depth) = set.depth()? else {
            continue;
        };
        let color = if color.is_some() {
            set.color()?.map(|frame| copy_rgb(&frame)).transpose()?
        } else {
            None
        };
        let infrared = if infrared.is_some() {
            set.infrared()?
                .map(|frame| copy_infrared(&frame))
                .transpose()?
        } else {
            None
        };
        count += 1;
        fps_count += 1;
        last_frame = Instant::now();
        if fps_start.elapsed() >= Duration::from_millis(500) {
            fps = fps_count as f32 / fps_start.elapsed().as_secs_f32();
            fps_count = 0;
            fps_start = Instant::now();
        }
        let capture = Capture {
            depth: Plane {
                size: [
                    depth.video().width()? as usize,
                    depth.video().height()? as usize,
                ],
                pixels: depth.samples()?.collect(),
            },
            scale_mm: depth.scale_mm()?,
            color,
            infrared,
            frame_index: depth.video().index()?,
            count,
            fps,
        };
        let mut state = shared.lock().unwrap();
        state.message = "Live".into();
        // One replaceable slot keeps latency and memory bounded if painting slows.
        state.latest = Some(capture);
        drop(state);
        ctx.request_repaint();
    }
    pipeline.stop()?;
    eprintln!("Viewer stopped after {count} framesets");
    Ok(())
}

fn preferred(profiles: &[VideoProfile]) -> Option<VideoProfile> {
    profiles
        .iter()
        .copied()
        .find(|p| p.width == 640 && p.fps == 30)
        .or_else(|| profiles.first().copied())
}

fn copy_rgb(frame: &VideoFrame<'_>) -> CaptureResult<Plane<u8>> {
    let size = [frame.width()? as usize, frame.height()? as usize];
    let data = frame.data()?;
    if frame.format()? != Format::RGB
        || size[0].checked_mul(size[1]).and_then(|n| n.checked_mul(3)) != Some(data.len())
    {
        return Err("RGB frame format or byte count is invalid".into());
    }
    Ok(Plane {
        size,
        pixels: data.to_vec(),
    })
}

fn copy_infrared(frame: &VideoFrame<'_>) -> CaptureResult<Plane<u16>> {
    let size = [frame.width()? as usize, frame.height()? as usize];
    let pixels = decode_infrared(frame.data()?, size, frame.format()?)?;
    Ok(Plane { size, pixels })
}

fn decode_infrared(data: &[u8], size: [usize; 2], format: Format) -> CaptureResult<Vec<u16>> {
    let len = size[0]
        .checked_mul(size[1])
        .filter(|&len| len > 0)
        .ok_or("Invalid IR dimensions")?;
    match format {
        Format::Y8 if data.len() == len => Ok(data.iter().map(|&p| u16::from(p)).collect()),
        Format::Y16 | Format::Y10 | Format::Y11 | Format::Y12 | Format::Y14
            if len.checked_mul(2) == Some(data.len()) =>
        {
            Ok(data
                .as_chunks::<2>()
                .0
                .iter()
                .copied()
                .map(u16::from_le_bytes)
                .collect())
        }
        _ => Err("IR frame is not a supported unpacked grayscale image".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn infrared_checks_format_size_and_endianness() {
        assert_eq!(
            decode_infrared(&[0x34, 0x12, 0xff, 0], [2, 1], Format::Y16).unwrap(),
            [0x1234, 255]
        );
        assert_eq!(
            decode_infrared(&[0, 255], [2, 1], Format::Y8).unwrap(),
            [0, 255]
        );
        assert!(decode_infrared(&[0; 3], [2, 1], Format::Y16).is_err());
        assert!(decode_infrared(&[0; 6], [2, 1], Format::RGB).is_err());
    }

    #[test]
    #[ignore = "requires exclusive access to one connected Orbbec camera"]
    fn worker_captures_owned_frames_and_releases_camera_on_stop() {
        for _ in 0..2 {
            let state = Arc::new(Mutex::new(CameraState::default()));
            let worker = Worker::spawn(egui::Context::default(), state.clone(), None);
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                {
                    let state = state.lock().unwrap();
                    assert!(!state.error, "{}", state.message);
                    if let Some(capture) = &state.latest
                        && capture.count >= 10
                    {
                        assert_eq!(
                            capture.depth.pixels.len(),
                            capture.depth.size[0] * capture.depth.size[1]
                        );
                        assert!(capture.scale_mm > 0.0);
                        let rgb = capture.color.as_ref().expect("Reference camera has RGB");
                        assert_eq!(rgb.pixels.len(), rgb.size[0] * rgb.size[1] * 3);
                        let ir = capture.infrared.as_ref().expect("Reference camera has IR");
                        assert_eq!(ir.pixels.len(), ir.size[0] * ir.size[1]);
                        break;
                    }
                }
                assert!(Instant::now() < deadline, "Capture worker timed out");
                thread::sleep(Duration::from_millis(20));
            }
            worker.stop();
            drop(worker);
        }
    }
}
