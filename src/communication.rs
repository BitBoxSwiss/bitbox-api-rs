// SPDX-License-Identifier: Apache-2.0

#[cfg(feature = "client")]
use super::u2fframing;
#[cfg(feature = "client")]
use crate::runtime::Runtime;
#[cfg(feature = "read-write")]
use crate::util::Threading;
#[cfg(feature = "read-write")]
use async_trait::async_trait;
use thiserror::Error;

pub const FIRMWARE_CMD: u8 = 0x80 + 0x40 + 0x01;

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
    #[error("u2f framing encoding error")]
    U2fEncode,
    #[error("error querying device info")]
    Info,
    #[error("firmware version {0} required")]
    Version(&'static str),
}

#[cfg(feature = "read-write")]
#[cfg_attr(feature = "multithreaded", async_trait)]
#[cfg_attr(not(feature="multithreaded"), async_trait(?Send))]
pub trait ReadWrite: Threading {
    fn write(&self, msg: &[u8]) -> Result<usize, Error>;
    async fn read(&self) -> Result<Vec<u8>, Error>;

    async fn query(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        self.write(msg)?;
        self.read().await
    }
}

#[cfg(feature = "client")]
pub struct U2fHidCommunication {
    read_write: Box<dyn ReadWrite>,
    u2fhid: u2fframing::U2fHid,
}

#[cfg(feature = "client")]
impl crate::util::Threading for U2fHidCommunication {}

#[cfg(feature = "client")]
impl U2fHidCommunication {
    pub fn from(read_write: Box<dyn ReadWrite>, cmd: u8) -> Self {
        U2fHidCommunication {
            read_write,
            u2fhid: u2fframing::U2fHid::new(cmd),
        }
    }
}

#[cfg(feature = "client")]
#[cfg_attr(feature = "multithreaded", async_trait)]
#[cfg_attr(not(feature="multithreaded"),async_trait(?Send))]
impl ReadWrite for U2fHidCommunication {
    fn write(&self, msg: &[u8]) -> Result<usize, Error> {
        let mut buf = [0u8; u2fframing::MAX_LEN];
        let size = self
            .u2fhid
            .encode(msg, &mut buf)
            .or(Err(Error::U2fEncode))?;
        for chunk in buf[..size].chunks(64) {
            self.read_write.write(chunk)?;
        }
        Ok(size)
    }

    async fn read(&self) -> Result<Vec<u8>, Error> {
        let mut readbuf = self.read_write.read().await?;
        loop {
            match self.u2fhid.decode(&readbuf).or(Err(Error::U2fDecode))? {
                Some(d) => {
                    return Ok(d);
                }
                None => {
                    let more = self.read_write.read().await?;
                    readbuf.extend_from_slice(&more);
                }
            }
        }
    }
}

// sinve v7.0.0, requets and responses are framed with hww* codes.
// hwwReq* are HWW-level framing opcodes of requests.
// New request.
pub const HWW_REQ_NEW: u8 = 0x00;
// Poll an outstanding request for completion.
pub const HWW_REQ_RETRY: u8 = 0x01;
// Cancel any outstanding request.
// const HWW_REQ_CANCEL: u8 = 0x02;
// Reset the previous session before starting a new one (since v9.28.0).
pub const HWW_REQ_RESET: u8 = 0x03;
// INFO api call (used to be OP_INFO api call), graduated to the toplevel framing so it works
// the same way for all firmware versions.
pub const HWW_INFO: u8 = b'i';

// hwwRsp* are HWW-level framing pocodes of responses.

// Request finished, payload is valid.
pub const HWW_RSP_ACK: u8 = 0x00;
// Request is outstanding, retry later with hwwOpRetry.
pub const HWW_RSP_NOTREADY: u8 = 0x01;
// Device is busy, request was dropped. Client should retry the exact same msg.
pub const HWW_RSP_BUSY: u8 = 0x02;
// Bad request.
pub const HWW_RSP_NACK: u8 = 0x03;

#[derive(Debug, PartialEq, Eq)]
pub enum HwwStatus<'a> {
    Ack(&'a [u8]),
    NotReady,
    Busy,
    Nack,
}

pub fn parse_hww_response(bytes: &[u8]) -> Result<HwwStatus<'_>, Error> {
    match bytes {
        [HWW_RSP_ACK, payload @ ..] => Ok(HwwStatus::Ack(payload)),
        [HWW_RSP_NOTREADY, ..] => Ok(HwwStatus::NotReady),
        [HWW_RSP_BUSY, ..] => Ok(HwwStatus::Busy),
        [HWW_RSP_NACK, ..] => Ok(HwwStatus::Nack),
        _ => Err(Error::Info),
    }
}

pub fn encode_hww_request(payload: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let out = out.get_mut(..=payload.len()).ok_or(Error::Write)?;
    out[0] = HWW_REQ_NEW;
    out[1..].copy_from_slice(payload);
    Ok(out.len())
}

pub fn needs_session_reset(version: &semver::Version) -> bool {
    *version >= semver::Version::new(9, 28, 0)
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum Product {
    Unknown,
    BitBox02Multi,
    BitBox02BtcOnly,
    BitBox02NovaMulti,
    BitBox02NovaBtcOnly,
}

#[derive(Debug)]
pub struct Info {
    pub version: semver::Version,
    pub product: Product,
    pub unlocked: bool,
    // Is None before firmware version 9.20.0.
    pub initialized: Option<bool>,
}

#[cfg(feature = "client")]
pub(crate) struct HwwCommunication<R: Runtime> {
    communication: Box<dyn ReadWrite>,
    pub info: Info,
    marker: std::marker::PhantomData<R>,
}

pub fn parse_info(response: &[u8]) -> Result<Info, Error> {
    let (&version_len, response) = response.split_first().ok_or(Error::Info)?;
    let (version_bytes, response) = response
        .split_at_checked(usize::from(version_len))
        .ok_or(Error::Info)?;
    let version_str = std::str::from_utf8(version_bytes)
        .or(Err(Error::Info))?
        .strip_prefix('v')
        .ok_or(Error::Info)?;

    let version = semver::Version::parse(version_str).or(Err(Error::Info))?;
    const PLATFORM_BITBOX02: u8 = 0x00;
    const PLATFORM_BITBOX02_NOVA: u8 = 0x02;
    const BITBOX02_EDITION_MULTI: u8 = 0x00;
    const BITBOX02_EDITION_BTCONLY: u8 = 0x01;
    let platform_byte = *response.first().ok_or(Error::Info)?;
    let edition_byte = *response.get(1).ok_or(Error::Info)?;
    let unlocked_byte = *response.get(2).ok_or(Error::Info)?;
    let initialized_byte = response.get(3);
    Ok(Info {
        version,
        product: match (platform_byte, edition_byte) {
            (PLATFORM_BITBOX02, BITBOX02_EDITION_MULTI) => Product::BitBox02Multi,
            (PLATFORM_BITBOX02, BITBOX02_EDITION_BTCONLY) => Product::BitBox02BtcOnly,
            (PLATFORM_BITBOX02_NOVA, BITBOX02_EDITION_MULTI) => Product::BitBox02NovaMulti,
            (PLATFORM_BITBOX02_NOVA, BITBOX02_EDITION_BTCONLY) => Product::BitBox02NovaBtcOnly,
            _ => Product::Unknown,
        },
        unlocked: match unlocked_byte {
            0x00 => false,
            0x01 => true,
            _ => return Err(Error::Info),
        },
        initialized: match initialized_byte {
            None => None,
            Some(0x00) => Some(false),
            Some(0x01) => Some(true),
            _ => return Err(Error::Info),
        },
    })
}

#[cfg(feature = "client")]
async fn reset_session<R: Runtime>(
    communication: &dyn ReadWrite,
    version: &semver::Version,
) -> Result<(), Error> {
    if !needs_session_reset(version) {
        return Ok(());
    }
    // Send at the framing layer so an unfinished workflow cannot consume the request.
    loop {
        match communication.query(&[HWW_REQ_RESET]).await?.as_slice() {
            [HWW_RSP_ACK] => return Ok(()),
            [HWW_RSP_BUSY] => R::sleep(std::time::Duration::from_secs(1)).await,
            _ => return Err(Error::Info),
        }
    }
}

#[cfg(feature = "client")]
impl<R: Runtime> HwwCommunication<R> {
    pub async fn from(communication: Box<dyn ReadWrite>) -> Result<Self, Error> {
        let info = parse_info(&communication.query(&[HWW_INFO]).await?)?;
        // communication message framing since 7.0.0
        if !semver::VersionReq::parse(">=7.0.0")
            .or(Err(Error::Unknown))?
            .matches(&info.version)
        {
            return Err(Error::Version(">=7.0.0"));
        }

        reset_session::<R>(communication.as_ref(), &info.version).await?;

        Ok(HwwCommunication {
            communication,
            info,
            marker: std::marker::PhantomData,
        })
    }

    #[cfg(all(test, not(feature = "multithreaded")))]
    pub(crate) fn from_test_parts(communication: Box<dyn ReadWrite>, info: Info) -> Self {
        HwwCommunication {
            communication,
            info,
            marker: std::marker::PhantomData,
        }
    }

    pub async fn query(&self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        let mut framed_msg = vec![0; msg.len() + 1];
        encode_hww_request(msg, &mut framed_msg)?;

        let mut response = loop {
            let response = self.communication.query(&framed_msg).await?;
            if matches!(parse_hww_response(&response)?, HwwStatus::Busy) {
                R::sleep(std::time::Duration::from_millis(1000)).await;
                continue;
            }
            break response;
        };
        loop {
            match parse_hww_response(&response)? {
                HwwStatus::Ack(_) => {
                    response.remove(0);
                    return Ok(response);
                }
                HwwStatus::Busy | HwwStatus::Nack => return Err(Error::Info),
                HwwStatus::NotReady => {
                    R::sleep(std::time::Duration::from_millis(200)).await;
                    response = self.communication.query(&[HWW_REQ_RETRY]).await?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hww_request_uses_caller_buffer() {
        let mut out = [0xFF; 4];
        assert_eq!(encode_hww_request(&[0x61, 0x62], &mut out).unwrap(), 3);
        assert_eq!(out, [0, 0x61, 0x62, 0xFF]);
        assert!(matches!(
            encode_hww_request(&[0x61; 4], &mut out),
            Err(Error::Write),
        ));
        assert_eq!(out, [0, 0x61, 0x62, 0xFF]);
    }

    #[test]
    fn hww_ack_borrows_payload_and_statuses_are_distinct() {
        let data = [0, 0x61, 0x62];
        let HwwStatus::Ack(payload) = parse_hww_response(&data).unwrap() else {
            panic!("expected acknowledgement");
        };
        assert_eq!(payload, b"ab");
        assert!(std::ptr::eq(payload, &data[1..]));
        assert_eq!(parse_hww_response(&[1]).unwrap(), HwwStatus::NotReady);
        assert_eq!(parse_hww_response(&[2]).unwrap(), HwwStatus::Busy);
        assert_eq!(parse_hww_response(&[3]).unwrap(), HwwStatus::Nack);
        for invalid in [&[][..], &[4]] {
            assert!(matches!(parse_hww_response(invalid), Err(Error::Info)));
        }
    }

    #[test]
    fn info_preserves_product_and_optional_initialized_state() {
        let old = parse_info(b"\x07v9.19.0\x00\x00\x01").unwrap();
        assert_eq!(old.version, semver::Version::new(9, 19, 0));
        assert_eq!(old.product, Product::BitBox02Multi);
        assert!(old.unlocked);
        assert_eq!(old.initialized, None);
        assert!(!needs_session_reset(&old.version));

        let new = parse_info(b"\x07v9.28.0\x02\x01\x00\x01").unwrap();
        assert_eq!(new.product, Product::BitBox02NovaBtcOnly);
        assert!(!new.unlocked);
        assert_eq!(new.initialized, Some(true));
        assert!(needs_session_reset(&new.version));
        assert!(!needs_session_reset(&semver::Version::new(9, 27, 1)));

        for invalid in [
            &b"\x08v9.28.0"[..],
            &b"\x079.28.00\x02\x01\x00"[..],
            &b"\x07v9.28.0\x02\x01"[..],
            &b"\x07v9.28.0\x02\x01\x02"[..],
            &b"\x07v9.28.0\x02\x01\x00\x02"[..],
        ] {
            assert!(matches!(parse_info(invalid), Err(Error::Info)));
        }
    }
}
