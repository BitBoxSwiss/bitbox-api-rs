use super::u2fframing::{self, U2FFraming};
use crate::runtime::Runtime;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unknown error")]
    Unknown,
    #[error("write error")]
    Write,
    #[error("read error")]
    Read,
    #[error("u2f framing decoding error")]
    U2fDecode,
    #[error("error querying device info")]
    Info,
    #[error("firmware version {0} or later required")]
    MinVersion(&'static str),
}

#[cfg(not(feature = "enforce-send"))]
#[async_trait(?Send)]
pub trait ReadWrite {
    fn write(&self, msg: &[u8]) -> Result<usize, Error>;
    async fn read(&self) -> Result<Vec<u8>, Error>;
}

#[cfg(feature = "enforce-send")]
#[async_trait]
pub trait ReadWrite {
    fn write(&self, msg: &[u8]) -> Result<usize, Error>;
    async fn read(&self) -> Result<Vec<u8>, Error>;
}

pub struct U2fCommunication<T> {
    read_write: T,
    u2fhid: u2fframing::U2fHid,
}

impl<T: ReadWrite> U2fCommunication<T> {
    pub fn from(read_write: T, cmd: u8) -> Self {
        U2fCommunication {
            read_write,
            u2fhid: u2fframing::U2fHid::new(cmd),
        }
    }

    fn write(&self, msg: &[u8]) -> Result<usize, Error> {
        let mut buf = [0u8; u2fframing::MAX_LEN];
        let size = self.u2fhid.encode(msg, &mut buf).unwrap();
        for chunk in buf[..size].chunks(64) {
            self.read_write.write(chunk)?;
        }
        Ok(size)
    }

    async fn read(&self) -> Result<Vec<u8>, Error> {
        let mut buffer = Vec::<u8>::new();
        loop {
            let res = self.read_write.read().await?;
            buffer.extend_from_slice(&res);
            if let Some(d) = self.u2fhid.decode(&buffer).or(Err(Error::U2fDecode))? {
                return Ok(d);
            }
        }
    }

    pub async fn query(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        self.write(msg)?;
        self.read().await
    }
}

// sinve v7.0.0, requets and responses are framed with hww* codes.
// hwwReq* are HWW-level framing opcodes of requests.
// New request.
const HWW_REQ_NEW: u8 = 0x00;
// Poll an outstanding request for completion.
const HWW_REQ_RETRY: u8 = 0x01;
// Cancel any outstanding request.
// const HWW_REQ_CANCEL: u8 = 0x02;
// INFO api call (used to be OP_INFO api call), graduated to the toplevel framing so it works
// the same way for all firmware versions.
const HWW_INFO: u8 = b'i';

// hwwRsp* are HWW-level framing pocodes of responses.

// Request finished, payload is valid.
const HWW_RSP_ACK: u8 = 0x00;
// Request is outstanding, retry later with hwwOpRetry.
const HWW_RSP_NOTREADY: u8 = 0x01;
// Device is busy, request was dropped. Client should retry the exact same msg.
const HWW_RSP_BUSY: u8 = 0x02;
// Bad request.
const HWW_RSP_NACK: u8 = 0x03;

#[derive(Debug)]
pub enum Product {
    Unknown,
    BitBox02Multi,
    BitBox02BtcOnly,
}

#[derive(Debug)]
pub struct Info {
    pub version: semver::Version,
    pub product: Product,
    pub unlocked: bool,
}

pub struct HwwCommunication<R: Runtime, T: ReadWrite> {
    communication: U2fCommunication<T>,
    pub info: Info,
    marker: std::marker::PhantomData<R>,
}

async fn get_info<T: ReadWrite>(communication: &U2fCommunication<T>) -> Result<Info, Error> {
    let response = communication.query(&[HWW_INFO]).await?;
    let (version_str_len, response) = (
        *response.first().ok_or(Error::Info)? as usize,
        response.get(1..).ok_or(Error::Info)?,
    );
    let (version_bytes, response) = (
        response.get(..version_str_len).ok_or(Error::Info)?,
        response.get(version_str_len..).ok_or(Error::Info)?,
    );
    let version_str = std::str::from_utf8(version_bytes)
        .or(Err(Error::Info))?
        .strip_prefix('v')
        .ok_or(Error::Info)?;

    let version = semver::Version::parse(version_str).or(Err(Error::Info))?;
    const PLATFORM_BITBOX02: u8 = 0x00;
    const BITBOX02_EDITION_MULTI: u8 = 0x00;
    const BITBOX02_EDITION_BTCONLY: u8 = 0x01;
    let platform_byte = *response.first().ok_or(Error::Info)?;
    let edition_byte = *response.get(1).ok_or(Error::Info)?;
    let unlocked_byte = *response.get(2).ok_or(Error::Info)?;
    Ok(Info {
        version,
        product: match (platform_byte, edition_byte) {
            (PLATFORM_BITBOX02, BITBOX02_EDITION_MULTI) => Product::BitBox02Multi,
            (PLATFORM_BITBOX02, BITBOX02_EDITION_BTCONLY) => Product::BitBox02BtcOnly,
            _ => Product::Unknown,
        },
        unlocked: match unlocked_byte {
            0x00 => false,
            0x01 => true,
            _ => return Err(Error::Info),
        },
    })
}

impl<R: Runtime, T: ReadWrite> HwwCommunication<R, T> {
    pub async fn from(communication: U2fCommunication<T>) -> Result<Self, Error> {
        let info = get_info(&communication).await?;
        Ok(HwwCommunication {
            communication,
            info,
            marker: std::marker::PhantomData,
        })
    }

    pub async fn query(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        if !semver::VersionReq::parse(">=7.0.0")
            .or(Err(Error::Unknown))?
            .matches(&self.info.version)
        {
            // msg framing since 7.0.0
            return Err(Error::MinVersion("7.0.0"));
        }

        let mut framed_msg = Vec::from([HWW_REQ_NEW]);
        framed_msg.extend_from_slice(msg);

        let mut response = loop {
            let response = self.communication.query(&framed_msg).await?;
            if let Some(&HWW_RSP_BUSY) = response.first() {
                R::sleep(std::time::Duration::from_millis(1000)).await;
                continue;
            }
            break response;
        };
        loop {
            match response.first() {
                Some(&HWW_RSP_ACK) => {
                    return Ok(response.split_off(1));
                }
                Some(&HWW_RSP_BUSY) => {
                    return Err(Error::Info);
                }
                Some(&HWW_RSP_NACK) => {
                    return Err(Error::Info);
                }
                Some(&HWW_RSP_NOTREADY) => {
                    R::sleep(std::time::Duration::from_millis(200)).await;
                    response = self.communication.query(&[HWW_REQ_RETRY]).await?;
                }
                _ => return Err(Error::Info),
            }
        }
    }
}
