use crate::{
    Error, FrameSet, Pipeline, Result,
    error::ffi,
    handle::{Handle, owned},
};
use orbbec_v1_sys as sys;
use std::{marker::PhantomData, slice};

#[derive(Clone, Copy, Debug)]
pub struct CameraIntrinsics {
    pub fx: f32,
    pub fy: f32,
    pub cx: f32,
    pub cy: f32,
    pub width: u32,
    pub height: u32,
}

/// Organized XYZ in meters in the depth camera's coordinate system:
/// X right, Y down, Z forward. Invalid points are [0, 0, 0].
/// The owned buffer can be sent to a rendering thread.
#[derive(Debug)]
pub struct PointCloud {
    pub width: u32,
    pub height: u32,
    pub points: Vec<[f32; 3]>,
}

/// Native SDK XYZ conversion using the active pipeline's calibration.
/// Create after starting the pipeline and drop before stopping it.
///
/// ```compile_fail,E0502
/// use orbbec_v1::{Pipeline, PointCloudFilter, Result};
/// fn stop_with_live_filter(pipeline: &mut Pipeline) -> Result<()> {
///     let filter = PointCloudFilter::new(pipeline)?;
///     pipeline.stop()?;
///     println!("{:?}", filter.intrinsics());
///     Ok(())
/// }
/// ```
pub struct PointCloudFilter<'a> {
    handle: Handle<sys::ob_filter>,
    intrinsics: CameraIntrinsics,
    _pipeline: PhantomData<&'a Pipeline>,
}

impl<'a> PointCloudFilter<'a> {
    pub fn new(pipeline: &'a Pipeline) -> Result<Self> {
        if !pipeline.is_streaming() {
            return Err(Error::new(
                "Start the pipeline before creating a point cloud filter",
            ));
        }
        let parameters = ffi!(ob_pipeline_get_camera_param, pipeline.ptr())?;
        let depth = parameters.depthIntrinsic;
        let intrinsics = CameraIntrinsics {
            fx: depth.fx,
            fy: depth.fy,
            cx: depth.cx,
            cy: depth.cy,
            width: u32::try_from(depth.width)
                .map_err(|_| Error::new("Invalid calibrated image width"))?,
            height: u32::try_from(depth.height)
                .map_err(|_| Error::new("Invalid calibrated image height"))?,
        };
        if ![intrinsics.fx, intrinsics.fy, intrinsics.cx, intrinsics.cy]
            .iter()
            .all(|v| v.is_finite())
            || intrinsics.fx <= 0.0
            || intrinsics.fy <= 0.0
            || intrinsics.width == 0
            || intrinsics.height == 0
        {
            return Err(Error::new(
                "SDK did not provide valid depth camera intrinsics",
            ));
        }
        let handle = owned!(ffi!(ob_create_pointcloud_filter), ob_delete_filter)?;
        ffi!(
            ob_pointcloud_filter_set_camera_param,
            handle.ptr(),
            parameters
        )?;
        ffi!(
            ob_pointcloud_filter_set_point_format,
            handle.ptr(),
            sys::OB_FORMAT_POINT
        )?;
        // Keep the v1 filter's default camera axes (X right, Y down, Z forward).
        // v1.10.37 declares a coordinate-system setter in its headers but does
        // not export that C symbol from its Linux library.
        Ok(Self {
            handle,
            intrinsics,
            _pipeline: PhantomData,
        })
    }

    pub fn intrinsics(&self) -> CameraIntrinsics {
        self.intrinsics
    }

    /// Process a frameset from the pipeline used to construct this filter.
    /// Coordinates are copied before releasing the SDK's output frame.
    pub fn process(&mut self, frames: &FrameSet<'_>) -> Result<Option<PointCloud>> {
        let Some(depth) = frames.depth()? else {
            return Ok(None);
        };
        let depth_scale_mm = depth.scale_mm()?;
        ffi!(
            ob_pointcloud_filter_set_position_data_scale,
            self.handle.ptr(),
            depth_scale_mm
        )?;
        let pointer = ffi!(ob_filter_process, self.handle.ptr(), frames.ptr())?;
        if pointer.is_null() {
            return Ok(None);
        }
        // SAFETY: ob_filter_process transfers a new output-frame reference.
        let points = unsafe { Handle::new(pointer, sys::ob_delete_frame)? };
        if ffi!(ob_frame_format, points.ptr())? != sys::OB_FORMAT_POINT {
            return Err(Error::new(
                "SDK point cloud filter did not return XYZ points",
            ));
        }
        let scale_mm = ffi!(ob_points_frame_get_position_value_scale, points.ptr())?;
        let len = ffi!(ob_frame_data_size, points.ptr())? as usize;
        if len == 0 || len > isize::MAX as usize {
            return Err(Error::new("Invalid point cloud buffer length"));
        }
        let pointer = ffi!(ob_frame_data, points.ptr())?;
        if pointer.is_null() {
            return Err(Error::new("Point cloud has a null data pointer"));
        }
        // SAFETY: The SDK reports this buffer length and the owning frame remains
        // alive until all bytes have been decoded into owned Rust coordinates.
        let bytes = unsafe { slice::from_raw_parts(pointer.cast::<u8>(), len) };
        decode_points(
            bytes,
            depth.video().width()?,
            depth.video().height()?,
            scale_mm,
        )
        .map(Some)
    }
}

fn decode_points(bytes: &[u8], width: u32, height: u32, scale_mm: f32) -> Result<PointCloud> {
    let count = (width as usize)
        .checked_mul(height as usize)
        .filter(|&n| n > 0)
        .ok_or_else(|| Error::new("Invalid point cloud dimensions"))?;
    if count.checked_mul(12) != Some(bytes.len()) || !scale_mm.is_finite() || scale_mm <= 0.0 {
        return Err(Error::new(
            "Point cloud XYZ byte count or position scale is invalid",
        ));
    }
    let scale_m = scale_mm / 1000.0;
    // Decode bytes instead of casting a potentially unaligned SDK buffer.
    let points = bytes
        .chunks_exact(12)
        .map(|bytes| {
            let point = std::array::from_fn(|axis| {
                let i = axis * 4;
                f32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) * scale_m
            });
            if point.iter().all(|v| v.is_finite()) && point[2] > 0.0 {
                point
            } else {
                [0.0; 3]
            }
        })
        .collect();
    Ok(PointCloud {
        width,
        height,
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xyz_decoding_preserves_axes_converts_units_and_rejects_invalid_points() {
        let values = [
            1000.0_f32,
            -2000.0,
            3000.0,
            0.0,
            0.0,
            0.0,
            f32::NAN,
            0.0,
            1000.0,
        ];
        let bytes: Vec<u8> = values.into_iter().flat_map(f32::to_le_bytes).collect();
        let cloud = decode_points(&bytes, 3, 1, 0.5).unwrap();
        for (actual, expected) in cloud.points[0].iter().zip([0.5, -1.0, 1.5]) {
            assert!((actual - expected).abs() < 1e-6);
        }
        assert_eq!(&cloud.points[1..], &[[0.0; 3], [0.0; 3]]);
        assert!(decode_points(&bytes[..35], 3, 1, 0.5).is_err());
        assert!(decode_points(&bytes, 3, 1, 0.0).is_err());
        assert!(decode_points(&bytes, 3, 1, f32::NAN).is_err());
    }
}
