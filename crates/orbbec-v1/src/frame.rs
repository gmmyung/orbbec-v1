use crate::{Error, Format, Pipeline, Result, error::ffi, handle::Handle};
use orbbec_v1_sys as sys;
use std::{marker::PhantomData, slice};

pub struct FrameSet<'a> {
    handle: Handle<sys::ob_frame>,
    _pipeline: PhantomData<&'a Pipeline>,
}

impl<'a> FrameSet<'a> {
    pub(crate) unsafe fn from_raw(pointer: *mut sys::ob_frame) -> Result<Self> {
        // SAFETY: Pipeline::wait transfers a newly owned frameset reference.
        let handle = unsafe { Handle::new(pointer, sys::ob_delete_frame)? };
        Ok(Self {
            handle,
            _pipeline: PhantomData,
        })
    }

    fn video_frame(&self, kind: sys::ob_frame_type) -> Result<Option<VideoFrame<'_>>> {
        let pointer = ffi!(ob_frameset_get_frame, self.handle.ptr(), kind)?;
        if pointer.is_null() {
            return Ok(None);
        }
        // SAFETY: The v1 C sample releases child frames separately from framesets.
        // Every getter returns a new reference; this view also borrows the set.
        let handle = unsafe { Handle::new(pointer, sys::ob_delete_frame)? };
        Ok(Some(VideoFrame {
            handle,
            _owner: PhantomData,
        }))
    }

    pub fn depth(&self) -> Result<Option<DepthFrame<'_>>> {
        Ok(self
            .video_frame(sys::OB_FRAME_DEPTH)?
            .map(|frame| DepthFrame { frame }))
    }
    pub fn color(&self) -> Result<Option<VideoFrame<'_>>> {
        self.video_frame(sys::OB_FRAME_COLOR)
    }
    pub fn infrared(&self) -> Result<Option<VideoFrame<'_>>> {
        self.video_frame(sys::OB_FRAME_IR)
    }
    pub fn infrared_left(&self) -> Result<Option<VideoFrame<'_>>> {
        self.video_frame(sys::OB_FRAME_IR_LEFT)
    }
    pub fn infrared_right(&self) -> Result<Option<VideoFrame<'_>>> {
        self.video_frame(sys::OB_FRAME_IR_RIGHT)
    }
}

pub struct VideoFrame<'a> {
    handle: Handle<sys::ob_frame>,
    _owner: PhantomData<&'a Pipeline>,
}

impl VideoFrame<'_> {
    pub fn width(&self) -> Result<u32> {
        ffi!(ob_video_frame_width, self.handle.ptr())
    }
    pub fn height(&self) -> Result<u32> {
        ffi!(ob_video_frame_height, self.handle.ptr())
    }
    pub fn format(&self) -> Result<Format> {
        Ok(Format(ffi!(ob_frame_format, self.handle.ptr())?))
    }
    pub fn index(&self) -> Result<u64> {
        ffi!(ob_frame_index, self.handle.ptr())
    }
    /// Device timestamp in microseconds; its clock domain may differ by device.
    pub fn timestamp_us(&self) -> Result<u64> {
        ffi!(ob_frame_time_stamp_us, self.handle.ptr())
    }
    pub fn system_timestamp_us(&self) -> Result<u64> {
        ffi!(ob_frame_system_time_stamp_us, self.handle.ptr())
    }

    /// Immutable SDK bytes, valid only while this frame remains borrowed.
    ///
    /// ```compile_fail,E0505
    /// use orbbec_v1::{FrameSet, Result};
    /// fn release_with_borrowed_bytes(set: &FrameSet<'_>) -> Result<()> {
    ///     let depth = set.depth()?.unwrap();
    ///     let bytes = depth.video().data()?;
    ///     drop(depth);
    ///     println!("{}", bytes.len());
    ///     Ok(())
    /// }
    /// ```
    pub fn data(&self) -> Result<&[u8]> {
        let len = ffi!(ob_frame_data_size, self.handle.ptr())? as usize;
        if len == 0 {
            return Ok(&[]);
        }
        if len > isize::MAX as usize {
            return Err(Error::new("Frame buffer exceeds Rust slice limit"));
        }
        let pointer = ffi!(ob_frame_data, self.handle.ptr())?;
        if pointer.is_null() {
            return Err(Error::new("Nonempty frame has a null data pointer"));
        }
        // SAFETY: The SDK owns a buffer of this length until the frame is released.
        // No mutable buffer view is exposed, and the slice borrows this frame.
        Ok(unsafe { slice::from_raw_parts(pointer.cast::<u8>(), len) })
    }
}

pub struct DepthFrame<'a> {
    frame: VideoFrame<'a>,
}

impl<'a> DepthFrame<'a> {
    pub fn video(&self) -> &VideoFrame<'a> {
        &self.frame
    }

    /// SDK depth multiplier: a sample multiplied by this value gives millimeters.
    pub fn scale_mm(&self) -> Result<f32> {
        let scale = ffi!(ob_depth_frame_get_value_scale, self.frame.handle.ptr())?;
        if !scale.is_finite() || scale <= 0.0 {
            return Err(Error::new("Invalid SDK depth scale"));
        }
        Ok(scale)
    }

    /// Decode the default unpacked 16-bit buffer, including Y11/Y12 on the
    /// Stereo S U3. Validate format AND actual byte length before interpreting it.
    /// Values are decoded as little-endian bytes without alignment assumptions.
    pub fn samples(&self) -> Result<impl ExactSizeIterator<Item = u16> + '_> {
        let format = self.frame.format()?;
        let width = self.frame.width()?;
        let height = self.frame.height()?;
        let data = self.frame.data()?;
        validate_depth_layout(format, width, height, data.len())?;
        Ok(data
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]])))
    }

    /// Zero samples mean invalid depth and return None. Coordinates are checked.
    pub fn distance_mm(&self, x: u32, y: u32) -> Result<Option<f32>> {
        let width = self.frame.width()?;
        let height = self.frame.height()?;
        if x >= width || y >= height {
            return Err(Error::new("Depth coordinates are outside the frame"));
        }
        let index = u64::from(y) * u64::from(width) + u64::from(x);
        let index = usize::try_from(index).map_err(|_| Error::new("Depth index overflow"))?;
        let raw = self
            .samples()?
            .nth(index)
            .ok_or_else(|| Error::new("Depth sample missing"))?;
        Ok(if raw == 0 {
            None
        } else {
            Some(f32::from(raw) * self.scale_mm()?)
        })
    }
}

fn validate_depth_layout(format: Format, width: u32, height: u32, len: usize) -> Result<()> {
    if !format.is_unpacked_depth() {
        return Err(Error::new(format!(
            "Unsupported depth sample format: {format}"
        )));
    }
    let expected = u64::from(width) * u64::from(height);
    let expected = expected
        .checked_mul(2)
        .ok_or_else(|| Error::new("Depth dimensions overflow"))?;
    if width == 0 || height == 0 || u64::try_from(len).ok() != Some(expected) {
        return Err(Error::new(
            "Depth data is not an unpacked width*height*2 buffer",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depth_layout_accepts_unpacked_legacy_formats_and_rejects_packed_data() {
        assert!(validate_depth_layout(Format::Y11, 640, 400, 512_000).is_ok());
        assert!(validate_depth_layout(Format::Y12, 640, 400, 512_000).is_ok());
        assert!(validate_depth_layout(Format::Y11, 640, 400, 352_000).is_err());
        assert!(validate_depth_layout(Format::RGB, 640, 400, 512_000).is_err());
        assert!(validate_depth_layout(Format::Y16, u32::MAX, u32::MAX, 0).is_err());
        assert!(validate_depth_layout(Format::Y16, 0, 0, 0).is_err());
    }
}
