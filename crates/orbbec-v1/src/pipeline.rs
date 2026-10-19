use crate::{
    Device, Error, FrameSet, Result,
    error::ffi,
    handle::{Handle, owned},
};
use orbbec_v1_sys as sys;
use std::{fmt, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sensor {
    Depth,
    Color,
    Infrared,
    InfraredLeft,
    InfraredRight,
}

impl Sensor {
    pub(crate) fn native(self) -> sys::ob_sensor_type {
        match self {
            Self::Depth => sys::OB_SENSOR_DEPTH,
            Self::Color => sys::OB_SENSOR_COLOR,
            Self::Infrared => sys::OB_SENSOR_IR,
            Self::InfraredLeft => sys::OB_SENSOR_IR_LEFT,
            Self::InfraredRight => sys::OB_SENSOR_IR_RIGHT,
        }
    }
    fn stream(self) -> sys::ob_stream_type {
        match self {
            Self::Depth => sys::OB_STREAM_DEPTH,
            Self::Color => sys::OB_STREAM_COLOR,
            Self::Infrared => sys::OB_STREAM_IR,
            Self::InfraredLeft => sys::OB_STREAM_IR_LEFT,
            Self::InfraredRight => sys::OB_STREAM_IR_RIGHT,
        }
    }
}

/// SDK format identifier, preserving unknown values without enum conversion UB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format(pub u32);

impl Format {
    pub const Y16: Self = Self(sys::OB_FORMAT_Y16);
    pub const Y8: Self = Self(sys::OB_FORMAT_Y8);
    pub const Y10: Self = Self(sys::OB_FORMAT_Y10);
    pub const Y11: Self = Self(sys::OB_FORMAT_Y11);
    pub const Y12: Self = Self(sys::OB_FORMAT_Y12);
    pub const Y14: Self = Self(sys::OB_FORMAT_Y14);
    pub const RGB: Self = Self(sys::OB_FORMAT_RGB);
    pub const MJPG: Self = Self(sys::OB_FORMAT_MJPG);
    pub(crate) fn is_unpacked_depth(self) -> bool {
        matches!(
            self.0,
            sys::OB_FORMAT_Y16
                | sys::OB_FORMAT_Z16
                | sys::OB_FORMAT_Y10
                | sys::OB_FORMAT_Y11
                | sys::OB_FORMAT_Y12
                | sys::OB_FORMAT_Y14
                | sys::OB_FORMAT_RLE
                | sys::OB_FORMAT_RVL
        )
    }
}
impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.0 {
            sys::OB_FORMAT_Y16 => "Y16",
            sys::OB_FORMAT_Y11 => "Y11",
            sys::OB_FORMAT_Y12 => "Y12",
            sys::OB_FORMAT_Y8 => "Y8",
            sys::OB_FORMAT_Y10 => "Y10",
            sys::OB_FORMAT_Y14 => "Y14",
            sys::OB_FORMAT_RGB => "RGB",
            sys::OB_FORMAT_BGR => "BGR",
            sys::OB_FORMAT_MJPG => "MJPG",
            sys::OB_FORMAT_YUYV => "YUYV",
            sys::OB_FORMAT_YUY2 => "YUY2",
            sys::OB_FORMAT_UYVY => "UYVY",
            _ => return write!(f, "Format({})", self.0),
        };
        f.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoProfile {
    pub sensor: Sensor,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: Format,
}
impl fmt::Display for VideoProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}: {}x{} @{} {}",
            self.sensor, self.width, self.height, self.fps, self.format
        )
    }
}

/// A pipeline retains its device and context and stops on Drop.
/// Frames borrow the pipeline, preventing a stop while their data is in use:
///
/// ```compile_fail,E0502
/// use orbbec_v1::{Pipeline, Result};
/// use std::time::Duration;
/// fn stop_with_live_frame(pipeline: &mut Pipeline) -> Result<()> {
///     let set = pipeline.wait(Duration::from_secs(1))?.unwrap();
///     let depth = set.depth()?.unwrap();
///     pipeline.stop()?;
///     println!("{}", depth.video().data()?.len());
///     Ok(())
/// }
/// ```
pub struct Pipeline {
    handle: Handle<sys::ob_pipeline>,
    config: Option<Handle<sys::ob_config>>,
    _device: Device,
    streaming: bool,
}

impl Pipeline {
    pub fn new(device: &Device) -> Result<Self> {
        let handle = owned!(
            ffi!(ob_create_pipeline_with_device, device.ptr()),
            ob_delete_pipeline
        )?;
        Ok(Self {
            handle,
            config: None,
            _device: device.clone(),
            streaming: false,
        })
    }

    pub(crate) fn ptr(&self) -> *mut sys::ob_pipeline {
        self.handle.ptr()
    }

    pub fn profiles(&self, sensor: Sensor) -> Result<Vec<VideoProfile>> {
        let list = owned!(
            ffi!(
                ob_pipeline_get_stream_profile_list,
                self.ptr(),
                sensor.native()
            ),
            ob_delete_stream_profile_list
        )?;
        let count = ffi!(ob_stream_profile_list_count, list.ptr())?;
        let mut profiles = Vec::new();
        for i in 0..count {
            let index = i32::try_from(i).map_err(|_| Error::new("Too many stream profiles"))?;
            let profile = owned!(
                ffi!(ob_stream_profile_list_get_profile, list.ptr(), index),
                ob_delete_stream_profile
            )?;
            profiles.push(VideoProfile {
                sensor,
                width: ffi!(ob_video_stream_profile_width, profile.ptr())?,
                height: ffi!(ob_video_stream_profile_height, profile.ptr())?,
                fps: ffi!(ob_video_stream_profile_fps, profile.ptr())?,
                format: Format(ffi!(ob_stream_profile_format, profile.ptr())?),
            });
        }
        Ok(profiles)
    }

    /// Start with explicit stream profiles. No hardware frame sync is requested.
    /// A frameset is requested only when all enabled streams are present; this
    /// does not imply synchronized exposures on legacy cameras.
    pub fn start(&mut self, profiles: &[VideoProfile]) -> Result<()> {
        if self.streaming {
            return Err(Error::new("Pipeline is already streaming"));
        }
        if profiles.is_empty() {
            return Err(Error::new("Enable at least one stream"));
        }
        for (i, profile) in profiles.iter().enumerate() {
            if profiles[..i].iter().any(|p| p.sensor == profile.sensor) {
                return Err(Error::new("A sensor may be enabled only once"));
            }
            if [profile.width, profile.height, profile.fps]
                .iter()
                .any(|&v| v == 0 || v > i32::MAX as u32)
            {
                return Err(Error::new(
                    "Stream dimensions and fps must be positive i32 values",
                ));
            }
        }
        let config = owned!(ffi!(ob_create_config), ob_delete_config)?;
        ffi!(ob_config_disable_all_stream, config.ptr())?;
        for profile in profiles {
            ffi!(
                ob_config_enable_video_stream,
                config.ptr(),
                profile.sensor.stream(),
                profile.width as i32,
                profile.height as i32,
                profile.fps as i32,
                profile.format.0
            )?;
        }
        ffi!(
            ob_config_set_frame_aggregate_output_mode,
            config.ptr(),
            sys::OB_FRAME_AGGREGATE_OUTPUT_FULL_FRAME_REQUIRE
        )?;
        self.config = Some(config);
        // A failed native start can partially start sensors. Retain configuration
        // and try stopping them; Drop retries if the SDK stop itself fails.
        self.streaming = true;
        if let Err(error) = ffi!(
            ob_pipeline_start_with_config,
            self.ptr(),
            self.config.as_ref().unwrap().ptr()
        ) {
            let _ = self.stop();
            return Err(error);
        }
        Ok(())
    }

    /// `Ok(None)` means the SDK returned no frameset before the timeout.
    /// Native SDK errors, including a timeout reported as an error, are retained.
    pub fn wait(&self, timeout: Duration) -> Result<Option<FrameSet<'_>>> {
        if !self.streaming {
            return Err(Error::new("Start the pipeline before waiting for frames"));
        }
        let millis = timeout.as_millis();
        if millis == 0 || millis > u32::MAX as u128 {
            return Err(Error::new("Timeout must be between 1 ms and u32::MAX ms"));
        }
        let pointer = ffi!(ob_pipeline_wait_for_frameset, self.ptr(), millis as u32)?;
        if pointer.is_null() {
            return Ok(None);
        }
        // SAFETY: wait returns a new frame reference; FrameSet borrows this pipeline.
        Ok(Some(unsafe { FrameSet::from_raw(pointer)? }))
    }

    /// Explicit stop reports native errors. Drop also attempts stop automatically.
    pub fn stop(&mut self) -> Result<()> {
        if self.streaming {
            ffi!(ob_pipeline_stop, self.ptr())?;
            self.streaming = false;
            self.config = None;
        }
        Ok(())
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
