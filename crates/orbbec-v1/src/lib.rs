//! Synchronous capture for legacy Orbbec cameras using SDK v1.10.37.
//!
//! SDK handles stay on one thread. Frames borrow their capture owner, so stopping
//! a pipeline while a frame is in use is rejected by the Rust compiler.
mod device;
mod error;
mod frame;
mod handle;
mod pipeline;
mod point_cloud;

pub use device::{Context, Device, DeviceDescriptor, DeviceInfo};
pub use error::{Error, Result};
pub use frame::{DepthFrame, FrameSet, VideoFrame};
pub use pipeline::{Format, Pipeline, Sensor, VideoProfile};
pub use point_cloud::{CameraIntrinsics, PointCloud, PointCloudFilter};

use orbbec_v1_sys as sys;

/// Version of the SDK library actually loaded by the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdkVersion {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
}

impl std::fmt::Display for SdkVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

pub fn sdk_version() -> SdkVersion {
    // SAFETY: These SDK functions take no pointers and return scalar values.
    unsafe {
        SdkVersion {
            major: sys::ob_get_major_version(),
            minor: sys::ob_get_minor_version(),
            patch: sys::ob_get_patch_version(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::{assert_impl_all, assert_not_impl_any};
    assert_impl_all!(PointCloud: Send, Sync);
    assert_not_impl_any!(Context: Send, Sync);
    assert_not_impl_any!(Device: Send, Sync);
    assert_not_impl_any!(Pipeline: Send, Sync);
    assert_not_impl_any!(FrameSet<'static>: Send, Sync);
    assert_not_impl_any!(VideoFrame<'static>: Send, Sync);
    assert_not_impl_any!(DepthFrame<'static>: Send, Sync);
    assert_not_impl_any!(PointCloudFilter<'static>: Send, Sync);

    #[test]
    fn loaded_sdk_is_the_pinned_version() {
        assert_eq!(
            sdk_version(),
            SdkVersion {
                major: 1,
                minor: 10,
                patch: 37
            }
        );
    }
}
