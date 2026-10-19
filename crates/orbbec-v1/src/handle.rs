use crate::{Error, Result, error::call};
use orbbec_v1_sys as sys;
use std::{marker::PhantomData, ptr::NonNull, rc::Rc};

pub(crate) struct Handle<T> {
    ptr: NonNull<T>,
    delete: unsafe extern "C" fn(*mut T, *mut *mut sys::ob_error),
    _thread: PhantomData<Rc<()>>,
}

impl<T> Handle<T> {
    /// The pointer must be a newly owned SDK handle with the given destructor.
    pub(crate) unsafe fn new(
        ptr: *mut T,
        delete: unsafe extern "C" fn(*mut T, *mut *mut sys::ob_error),
    ) -> Result<Self> {
        Ok(Self {
            ptr: NonNull::new(ptr)
                .ok_or_else(|| Error::new("SDK returned an unexpected null handle"))?,
            delete,
            _thread: PhantomData,
        })
    }
    pub(crate) fn ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

impl<T> Drop for Handle<T> {
    fn drop(&mut self) {
        // SAFETY: Each Handle exclusively owns one SDK reference. Parent handles
        // are retained by the public wrapper until this destructor completes.
        let _ = call(|error| unsafe { (self.delete)(self.ptr(), error) });
    }
}

macro_rules! owned {
    ($result:expr, $delete:ident) => {{
        let pointer = $result?;
        // SAFETY: SDK factories/getters at invocation sites return a new owned
        // reference, released with the corresponding SDK destructor exactly once.
        unsafe { crate::handle::Handle::new(pointer, orbbec_v1_sys::$delete) }
    }};
}
pub(crate) use owned;
