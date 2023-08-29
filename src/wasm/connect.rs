use super::{noise, BitBox, JavascriptError};
use wasm_bindgen::prelude::*;

struct JsReadWrite {
    write_function: js_sys::Function,
    read_function: js_sys::Function,
}
use crate::communication;

#[wasm_bindgen(raw_module = "./webhid")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn getWebHIDDevice(vendorId: f64, productId: f64) -> Result<JsValue, JsValue>;
}

#[async_trait::async_trait(?Send)]
impl communication::ReadWrite for JsReadWrite {
    fn write(&self, msg: &[u8]) -> Result<usize, communication::Error> {
        self.write_function
            .call1(&JsValue::NULL, &js_sys::Uint8Array::from(msg))
            .map_err(|_| communication::Error::Write)?;
        Ok(msg.len())
    }

    async fn read(&self) -> Result<Vec<u8>, communication::Error> {
        let result = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(
            self.read_function
                .call0(&JsValue::NULL)
                .map_err(|_| communication::Error::Read)?,
        ))
        .await
        .unwrap();
        Ok(js_sys::Uint8Array::from(result).to_vec())
    }
}

#[wasm_bindgen(js_name = bitbox02ConnectWebHID)]
pub async fn bitbox02_connect_webhid() -> Result<BitBox, JavascriptError> {
    let result = getWebHIDDevice(
        crate::constants::VENDOR_ID as _,
        crate::constants::PRODUCT_ID as _,
    )
    .await
    .map_err(|_| JavascriptError::CouldNotOpen)?;
    if result.is_null() {
        return Err(JavascriptError::UserAbort);
    }
    let write_function: js_sys::Function = js_sys::Reflect::get(&result, &"write".into())
        .or(Err(JavascriptError::InvalidType("`write` key missing")))?
        .dyn_into()
        .or(Err(JavascriptError::InvalidType(
            "`write` object is not a function",
        )))?;
    let read_function: js_sys::Function = js_sys::Reflect::get(&result, &"read".into())
        .or(Err(JavascriptError::InvalidType("`read` key missing")))?
        .dyn_into()
        .or(Err(JavascriptError::InvalidType(
            "`read` object is not a function",
        )))?;

    let read_write = Box::new(JsReadWrite {
        write_function,
        read_function,
    });
    let communication = Box::new(communication::U2fHidCommunication::from(
        read_write,
        communication::FIRMWARE_CMD,
    ));

    Ok(BitBox(
        crate::BitBox::from(communication, Box::new(noise::LocalStorageNoiseConfig {})).await?,
    ))
}
