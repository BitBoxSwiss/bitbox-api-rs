use super::constants::{PRODUCT_ID, VENDOR_ID};
use async_trait::async_trait;
use thiserror::Error;

use super::communication::{
    Error as CommuincationError, ReadWrite, U2fCommunication, FIRMWARE_CMD,
};

/// The hid product string of the multi edition firmware.
const FIRMWARE_PRODUCT_STRING_MULTI: &str = "BitBox02";
/// The hid product string of the btc-only edition firmware.
const FIRMWARE_PRODUCT_STRING_BTCONLY: &str = "BitBox02BTC";

#[async_trait(?Send)]
impl ReadWrite for hidapi::HidDevice {
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
pub fn get_any_bitbox02() -> Result<Box<dyn ReadWrite>, UsbError> {
    let api = hidapi::HidApi::new().unwrap();
    for device_info in api.device_list() {
        if is_bitbox02(device_info) {
            let device = Box::new(device_info.open_device(&api)?);
            let communication = Box::new(U2fCommunication::from(device, FIRMWARE_CMD));
            return Ok(communication);
        }
    }
    Err(UsbError::NotFound)
}
