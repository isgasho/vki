use crate::imp::serial::Serial;
use crate::imp::{DeviceInner, FenceInner};
use crate::{Error, Fence, FenceError};

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

// TODO: The GPUWeb definition of a Fence seems to be more akin to a Vulkan event, but this
//       implementation doesn't match GPUWeb anyway. Also, MokenVK doesn't support events
//       so we'll use this gimmick implementation for now.

impl Fence {
    pub fn reset(&self) -> Result<(), Error> {
        *self.inner.serial.lock() = get_last_submitted_serial(&self.inner.device);
        Ok(())
    }

    pub fn wait(&self, timeout: Duration) -> Result<bool, FenceError> {
        let timeout = Instant::now() + timeout;
        let serial = *self.inner.serial.lock();
        let mut stalled = false;
        while serial > get_last_completed_serial(&self.inner.device) {
            if Instant::now() >= timeout {
                return Err(FenceError::Timeout);
            }
            if stalled {
                std::thread::yield_now();
            } else {
                stalled = true;
            }
            self.inner.device.tick()?;
        }
        Ok(stalled)
    }

    pub fn is_signaled(&self) -> bool {
        let serial = *self.inner.serial.lock();
        serial <= get_last_completed_serial(&self.inner.device)
    }
}

impl FenceInner {
    pub fn new(device: Arc<DeviceInner>) -> Result<FenceInner, Error> {
        let serial = { Mutex::new(get_last_submitted_serial(&device)) };
        Ok(FenceInner { serial, device })
    }
}

fn get_last_submitted_serial(device: &DeviceInner) -> Serial {
    let state = device.state.lock();
    state.get_last_submitted_serial()
}

fn get_last_completed_serial(device: &DeviceInner) -> Serial {
    let state = device.state.lock();
    state.get_last_completed_serial()
}

impl Into<Fence> for FenceInner {
    fn into(self) -> Fence {
        Fence { inner: self }
    }
}
