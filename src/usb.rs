use super::constants::{PRODUCT_ID, VENDOR_ID};
use async_trait::async_trait;
use thiserror::Error;

#[cfg(feature = "multithreaded")]
use std::sync::Mutex;

use super::communication::{
    Error as CommunicationError, ReadWrite, U2fHidCommunication, FIRMWARE_CMD,
};

/// The hid product string of the multi edition firmware.
const FIRMWARE_PRODUCT_STRING_MULTI: &str = "BitBox02";
/// The hid product string of the btc-only edition firmware.
const FIRMWARE_PRODUCT_STRING_BTCONLY: &str = "BitBox02BTC";

#[cfg(not(feature = "multithreaded"))]
#[async_trait(?Send)]
impl ReadWrite for hidapi::HidDevice {
    fn write(&self, msg: &[u8]) -> Result<usize, CommunicationError> {
        let mut v = vec![0x00];
        v.extend_from_slice(msg);
        hidapi::HidDevice::write(self, &v).or(Err(CommunicationError::Write))
    }

    async fn read(&self) -> Result<Vec<u8>, CommunicationError> {
        let mut buf = [0u8; 64];
        let res = hidapi::HidDevice::read(self, &mut buf).or(Err(CommunicationError::Read))?;
        Ok(buf[..res].to_vec())
    }
}

#[cfg(feature = "multithreaded")]
pub struct MultithreadedHidDevice(Mutex<hidapi::HidDevice>);

#[cfg(feature = "multithreaded")]
#[async_trait]
impl ReadWrite for MultithreadedHidDevice {
    fn write(&self, msg: &[u8]) -> Result<usize, CommunicationError> {
        let mut device = self.0.lock().unwrap();
        let mut v = vec![0x00];
        v.extend_from_slice(msg);
        hidapi::HidDevice::write(&mut device, &v).or(Err(CommunicationError::Write))
    }

    async fn read(&self) -> Result<Vec<u8>, CommunicationError> {
        let device = self.0.lock().unwrap();
        let mut buf = [0u8; 64];
        let res = hidapi::HidDevice::read(&device, &mut buf).or(Err(CommunicationError::Read))?;
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
#[cfg(not(feature = "multithreaded"))]
pub fn get_any_bitbox02() -> Result<Box<dyn ReadWrite>, UsbError> {
    let api = hidapi::HidApi::new().unwrap();
    for device_info in api.device_list() {
        if is_bitbox02(device_info) {
            let device = device_info.open_device(&api)?;
            let communication = Box::new(U2fHidCommunication::from(device, FIRMWARE_CMD));
            return Ok(communication);
        }
    }
    Err(UsbError::NotFound)
}

#[cfg(feature = "multithreaded")]
pub fn get_any_bitbox02() -> Result<Box<dyn ReadWrite>, UsbError> {
    let api = hidapi::HidApi::new().unwrap();
    for device_info in api.device_list() {
        if is_bitbox02(device_info) {
            let device = MultithreadedHidDevice(Mutex::new(device_info.open_device(&api)?));
            let communication = Box::new(U2fHidCommunication::from(device, FIRMWARE_CMD));
            return Ok(communication);
        }
    }
    Err(UsbError::NotFound)
}
