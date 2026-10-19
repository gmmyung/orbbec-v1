use crate::{
    Error, Result,
    error::{ffi, sdk_string},
    handle::{Handle, owned},
    sdk_version,
};
use orbbec_v1_sys as sys;
use std::{ffi::CString, rc::Rc};

#[derive(Debug, Clone)]
pub struct DeviceDescriptor {
    pub name: String,
    pub serial: String,
    pub uid: String,
    pub vid: i32,
    pub pid: i32,
    pub connection: String,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub serial: String,
    pub uid: String,
    pub firmware: String,
}

/// SDK context. Clones share the same native context on the current thread.
#[derive(Clone)]
pub struct Context {
    handle: Rc<Handle<sys::ob_context>>,
}

impl Context {
    pub fn new() -> Result<Self> {
        let version = sdk_version();
        if (version.major, version.minor, version.patch) != (1, 10, 37) {
            return Err(Error::new(format!(
                "Expected SDK 1.10.37, loaded {version}"
            )));
        }
        if let Err(code) = sys::check_usb_backend() {
            return Err(Error::new(format!(
                "USB backend initialization failed (libusb error {code}); check access to the host USB subsystem"
            )));
        }
        ffi!(ob_set_logger_to_console, sys::OB_LOG_SEVERITY_WARN)?;
        let handle = owned!(ffi!(ob_create_context), ob_delete_context)?;
        Ok(Self {
            handle: Rc::new(handle),
        })
    }

    pub fn devices(&self) -> Result<Vec<DeviceDescriptor>> {
        let list = owned!(
            ffi!(ob_query_device_list, self.handle.ptr()),
            ob_delete_device_list
        )?;
        let count = ffi!(ob_device_list_device_count, list.ptr())?;
        let mut devices = Vec::new();
        for i in 0..count {
            // SAFETY: Strings are copied before dropping the owning list.
            let name = sdk_string!(ffi!(ob_device_list_get_device_name, list.ptr(), i));
            let serial = sdk_string!(ffi!(ob_device_list_get_device_serial_number, list.ptr(), i));
            let uid = sdk_string!(ffi!(ob_device_list_get_device_uid, list.ptr(), i));
            let connection = sdk_string!(ffi!(
                ob_device_list_get_device_connection_type,
                list.ptr(),
                i
            ));
            devices.push(DeviceDescriptor {
                name,
                serial,
                uid,
                connection,
                pid: ffi!(ob_device_list_get_device_pid, list.ptr(), i)?,
                vid: ffi!(ob_device_list_get_device_vid, list.ptr(), i)?,
            });
        }
        Ok(devices)
    }

    /// Open the selected USB path instead of relying on enumeration order.
    pub fn open(&self, device: &DeviceDescriptor) -> Result<Device> {
        let uid = CString::new(device.uid.as_str())
            .map_err(|_| Error::new("Device UID contains a NUL byte"))?;
        let list = owned!(
            ffi!(ob_query_device_list, self.handle.ptr()),
            ob_delete_device_list
        )?;
        let handle = owned!(
            ffi!(ob_device_list_get_device_by_uid, list.ptr(), uid.as_ptr()),
            ob_delete_device
        )?;
        Ok(Device {
            inner: Rc::new(DeviceInner {
                handle,
                _context: self.clone(),
            }),
        })
    }

    /// Some OpenNI devices expose their serial only after being opened. Inspect
    /// one device at a time instead of relying on the enumeration string.
    pub fn open_by_serial(&self, serial: &str) -> Result<Device> {
        if serial.is_empty() {
            return Err(Error::new("Serial number must not be empty"));
        }
        for descriptor in self.devices()? {
            let device = self.open(&descriptor)?;
            if device.info()?.serial == serial {
                return Ok(device);
            }
        }
        Err(Error::new(format!("No camera with serial {serial}")))
    }
}

struct DeviceInner {
    handle: Handle<sys::ob_device>,
    _context: Context,
}

#[derive(Clone)]
pub struct Device {
    inner: Rc<DeviceInner>,
}

impl Device {
    pub(crate) fn ptr(&self) -> *mut sys::ob_device {
        self.inner.handle.ptr()
    }

    pub fn info(&self) -> Result<DeviceInfo> {
        let info = owned!(
            ffi!(ob_device_get_device_info, self.ptr()),
            ob_delete_device_info
        )?;
        // SAFETY: All strings are copied while the DeviceInfo handle is alive.
        Ok(DeviceInfo {
            name: sdk_string!(ffi!(ob_device_info_name, info.ptr())),
            serial: sdk_string!(ffi!(ob_device_info_serial_number, info.ptr())),
            uid: sdk_string!(ffi!(ob_device_info_uid, info.ptr())),
            firmware: sdk_string!(ffi!(ob_device_info_firmware_version, info.ptr())),
        })
    }
}
