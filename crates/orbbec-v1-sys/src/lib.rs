//! Raw, unsafe bindings to Orbbec SDK **v1.10.37**. See the workspace README.
#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// SDK staging directory containing `lib/` and `config/` for this build's target.
///
/// Cargo arranges the loader path for `cargo run` and `cargo test`. For deployment,
/// copy these libraries and configuration alongside the executable and configure
/// its loader path. This build-time path is not a portable installation location.
pub fn sdk_directory() -> &'static std::path::Path {
    std::path::Path::new(env!("ORBBEC_V1_RESOLVED_SDK_DIR"))
}

#[link(name = "ob_usb")]
unsafe extern "C" {
    fn libusb_init(context: *mut *mut std::ffi::c_void) -> std::ffi::c_int;
    fn libusb_exit(context: *mut std::ffi::c_void);
}

/// Probe the SDK's bundled USB backend with a temporary context, opening no devices.
/// Some isolated hosts cannot initialize libusb; SDK v1 can crash on this path.
/// A successful probe does not guarantee subsequent SDK/device initialization.
pub fn check_usb_backend() -> Result<(), i32> {
    let mut context = std::ptr::null_mut();
    // SAFETY: libusb writes a newly owned context to this valid out-pointer.
    // On failure it cleans up internally; only a successful context is released.
    let status = unsafe { libusb_init(&mut context) };
    if status < 0 {
        return Err(status);
    }
    if context.is_null() {
        return Err(-99); // LIBUSB_ERROR_OTHER; never release the default context.
    }
    // SAFETY: No devices or references were obtained from this owned context.
    unsafe { libusb_exit(context) };
    Ok(())
}
