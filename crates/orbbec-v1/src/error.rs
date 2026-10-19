use orbbec_v1_sys as sys;
use std::{ffi::CStr, fmt, ptr};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub function: Option<String>,
    pub status: Option<u32>,
    pub exception_type: Option<u32>,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            function: None,
            status: None,
            exception_type: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(function) = &self.function {
            write!(f, "{function}: ")?;
        }
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}

/// Copy an SDK string while its owner is alive.
pub(crate) unsafe fn copy_string(p: *const std::ffi::c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: Callers provide a live, NUL-terminated SDK string.
    unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() }
}

pub(crate) fn call<T>(f: impl FnOnce(*mut *mut sys::ob_error) -> T) -> Result<T> {
    let mut error = ptr::null_mut();
    let value = f(&mut error);
    if error.is_null() {
        return Ok(value);
    }
    // SAFETY: The SDK allocated this error for this call. Copy all text before
    // freeing it, including when the error originates in a Drop implementation.
    unsafe {
        let result = Error {
            message: copy_string(sys::ob_error_message(error)),
            function: Some(copy_string(sys::ob_error_function(error))),
            status: Some(sys::ob_error_status(error)),
            exception_type: Some(sys::ob_error_exception_type(error)),
        };
        sys::ob_delete_error(error);
        Err(result)
    }
}

// Each invocation supplies only live handles and correctly typed arguments.
// A fresh error out-pointer is used for every call.
macro_rules! ffi {
    ($function:ident $(, $argument:expr)* $(,)?) => {
        crate::error::call(|error| {
            // SAFETY: Invocation sites uphold this SDK function's handle and
            // buffer preconditions; ownership is tracked by Handle and borrows.
            unsafe { orbbec_v1_sys::$function($($argument,)* error) }
        })
    };
}
pub(crate) use ffi;

macro_rules! sdk_string {
    ($call:expr) => {{
        let pointer = $call?;
        // SAFETY: Invocation sites retain the SDK owner while copying its string.
        unsafe { crate::error::copy_string(pointer) }
    }};
}
pub(crate) use sdk_string;
