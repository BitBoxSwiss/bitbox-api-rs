use super::constants::{PRODUCT_ID, VENDOR_ID};
use async_trait::async_trait;
use thiserror::Error;

#[cfg(feature = "enforce-send")]
use std::sync::Mutex;

use super::communication::{Error as CommuincationError, ReadWrite};

/// The hid product string of the multi edition firmware.
const FIRMWARE_PRODUCT_STRING_MULTI: &str = "BitBox02";
/// The hid product string of the btc-only edition firmware.
const FIRMWARE_PRODUCT_STRING_BTCONLY: &str = "BitBox02BTC";

#[cfg(not(feature = "enforce-send"))]
pub type Transport = hidapi::HidDevice;

#[cfg(not(feature = "enforce-send"))]
#[async_trait(?Send)]
impl ReadWrite for Transport {
    fn write(&self, msg: &[u8]) -> Result<usize, CommuincationError> {
        let mut v = vec![0x00];
        v.extend_from_slice(msg);
        hidapi::HidDevice::write(self, &v).or(Err(CommuincationError::Write))
    }

    async fn read(&self) -> Result<Vec<u8>, CommuincationError> {
        let mut buf = [0u8; 64];
        let res = hidapi::HidDevice::read(self, &mut buf).or(Err(CommuincationError::Read))?;
        Ok(buf[..res].to_vec())
    }
}

#[cfg(feature = "enforce-send")]
pub struct Transport(Mutex<hidapi::HidDevice>);

#[cfg(feature = "enforce-send")]
#[async_trait]
impl ReadWrite for Transport {
    fn write(&self, msg: &[u8]) -> Result<usize, CommuincationError> {
        let mut device = self.0.lock().unwrap();
        let mut v = vec![0x00];
        v.extend_from_slice(msg);
        hidapi::HidDevice::write(&mut device, &v).or(Err(CommuincationError::Write))
    }

    async fn read(&self) -> Result<Vec<u8>, CommuincationError> {
        let device = self.0.lock().unwrap();
        let mut buf = [0u8; 64];
        let res = hidapi::HidDevice::read(&device, &mut buf).or(Err(CommuincationError::Read))?;
        Ok(buf[..res].to_vec())
    }
}

#[cfg(feature = "enforce-send")]
impl From<hidapi::HidDevice> for Transport {
    fn from(device: hidapi::HidDevice) -> Transport {
        Transport(Mutex::new(device))
    }
}

#[derive(Error, Debug)]
pub enum UsbError {
    #[error("hid error: {0}")]
    Hid(#[from] hidapi::HidError),
    #[error("could not find device or device is busy")]
    NotFound,
}

fn is_bitbox02(device_info: &hidapi::DeviceInfo) -> bool {
    (matches!(
        device_info.product_string(),
        Some(FIRMWARE_PRODUCT_STRING_MULTI)
    ) || matches!(
        device_info.product_string(),
        Some(FIRMWARE_PRODUCT_STRING_BTCONLY)
    )) && device_info.vendor_id() == VENDOR_ID
        && device_info.product_id() == PRODUCT_ID
        && (device_info.usage_page() == 0xffff || device_info.interface_number() == 0)
}

/// Returns the first BitBox02 HID device that is found, or `Err(UsbError::NotFound)` if none is
/// available.
pub fn get_any_bitbox02() -> Result<Transport, UsbError> {
    let api = hidapi::HidApi::new().unwrap();
    for device_info in api.device_list() {
        if is_bitbox02(device_info) {
            return Ok(Transport::from(device_info.open_device(&api)?));
        }
    }
    Err(UsbError::NotFound)
}
