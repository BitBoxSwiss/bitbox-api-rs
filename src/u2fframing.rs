// SPDX-License-Identifier: Apache-2.0

use byteorder::BigEndian;
use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;
use std::io;
use std::io::Cursor;

const REPORT_LEN: usize = 64;
const HEADER_INIT_LEN: usize = 7;
const HEADER_CONT_LEN: usize = 5;
const PAYLOAD_INIT_LEN: usize = REPORT_LEN - HEADER_INIT_LEN;
const PAYLOAD_CONT_LEN: usize = REPORT_LEN - HEADER_CONT_LEN;
// U2F specs:
// With this approach, a message with a payload less or equal to (s - 7) may be sent as one packet.
// A larger message is then divided into one or more continuation packets, starting with sequence
// number 0, which then increments by one to a maximum of 127.
// With a packet size of 64 bytes (max for full-speed devices), this means that the maximum message
// payload length is 64 - 7 + 128 * (64 - 5) = 7609 bytes.
const MAX_PAYLOAD_LEN: usize = PAYLOAD_INIT_LEN + 128 * PAYLOAD_CONT_LEN;
// This is the buffer size needed to fit the largest possible u2f package with headers
pub const MAX_LEN: usize = 129 * 64;

pub fn parse_header(buf: &[u8]) -> io::Result<(u32, u8, u16)> {
    if buf.len() < HEADER_INIT_LEN {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Buffer to short to contain header (7 bytes)",
        ));
    }
    let mut rdr = Cursor::new(buf);
    let cid = rdr.read_u32::<BigEndian>()?;
    let cmd = rdr.read_u8()?;
    let len = rdr.read_u16::<BigEndian>()?;
    Ok((cid, cmd, len))
}

pub fn encode_header_init(cid: u32, cmd: u8, len: u16, mut buf: &mut [u8]) -> io::Result<usize> {
    if buf.len() < HEADER_INIT_LEN {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Buffer to short to contain header (7 bytes)",
        ));
    }
    buf.write_u32::<BigEndian>(cid)?;
    buf.write_u8(cmd)?;
    buf.write_u16::<BigEndian>(len)?;
    Ok(7)
}

pub fn encode_header_cont(cid: u32, seq: u8, mut buf: &mut [u8]) -> io::Result<usize> {
    if buf.len() < HEADER_CONT_LEN {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Buffer to short to contain header (5 bytes)",
        ));
    }
    buf.write_u32::<BigEndian>(cid)?;
    buf.write_u8(seq)?;
    Ok(5)
}

// TODO: Add randomness to CID
pub fn generate_cid() -> u32 {
    0xff00ff00
}

// U2fHid writes packets / usb reports. 64 bytes at a time
pub struct U2fHid {
    cid: u32,
    cmd: u8,
}

impl U2fHid {
    pub fn new(cmd: u8) -> Self {
        U2fHid {
            cid: generate_cid(),
            cmd,
        }
    }

    #[cfg(test)]
    pub fn with_cid(cid: u32, cmd: u8) -> Self {
        U2fHid { cid, cmd }
    }

    fn get_encoded_len(len: usize) -> usize {
        REPORT_LEN
            * (1 + len
                .saturating_sub(PAYLOAD_INIT_LEN)
                .div_ceil(PAYLOAD_CONT_LEN))
    }

    /// Encode function.
    pub fn encode(&self, mut message: &[u8], mut buf: &mut [u8]) -> io::Result<usize> {
        if message.len() > MAX_PAYLOAD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Message too long",
            ));
        }
        let enc_len = Self::get_encoded_len(message.len());
        if buf.len() < enc_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Message won't fit in buffer",
            ));
        }
        let len = encode_header_init(self.cid, self.cmd, message.len() as u16, buf)?;
        buf = &mut buf[len..];

        let len = PAYLOAD_INIT_LEN.min(message.len());
        let buf_slice = &mut buf[..len];
        buf_slice.copy_from_slice(&message[..len]);

        message = &message[len..];
        buf = &mut buf[len..];

        let mut seq = 0u8;
        while !message.is_empty() {
            let len = encode_header_cont(self.cid, seq, buf)?;
            buf = &mut buf[len..];

            let len = PAYLOAD_CONT_LEN.min(message.len());
            let buf_slice = &mut buf[..len];
            buf_slice.copy_from_slice(&message[..len]);
            buf = &mut buf[len..];
            message = &message[len..];

            seq += 1;
        }

        Ok(enc_len)
    }

    /// Decode a message from whole reports, ignoring extra reports after the message.
    /// Returns `Ok(None)` if more whole reports are needed.
    pub fn decode(&self, buf: &[u8]) -> io::Result<Option<Vec<u8>>> {
        if buf.is_empty() || !buf.chunks_exact(REPORT_LEN).remainder().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete U2F report",
            ));
        }
        let (cid, cmd, len) = parse_header(buf)?;
        if cid != self.cid {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Wrong CID",
            ));
        }
        if cmd != self.cmd {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Wrong CMD",
            ));
        }
        let len = usize::from(len);
        if len > MAX_PAYLOAD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Message too long",
            ));
        }

        let enc_len = Self::get_encoded_len(len);
        if buf.len() < enc_len {
            return Ok(None);
        }

        let mut res = Vec::with_capacity(len);
        res.extend_from_slice(&buf[HEADER_INIT_LEN..HEADER_INIT_LEN + PAYLOAD_INIT_LEN.min(len)]);
        for (seq, report) in buf[..enc_len].chunks_exact(REPORT_LEN).skip(1).enumerate() {
            if report[..4] != buf[..4] {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Wrong CID"));
            }
            if report[4] != seq as u8 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Wrong continuation sequence",
                ));
            }
            let take = PAYLOAD_CONT_LEN.min(len - res.len());
            res.extend_from_slice(&report[HEADER_CONT_LEN..HEADER_CONT_LEN + take]);
        }
        Ok(Some(res))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u2fhid_payload_length_limit() {
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        let mut buf = [0xAA; MAX_LEN];
        for len in [7610, 65536] {
            assert_eq!(
                codec.encode(&vec![0; len], &mut buf).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
            );
            assert!(buf.iter().all(|&byte| byte == 0xAA));
        }
        assert_eq!(
            codec.encode(&[0; 58], &mut buf[..64]).unwrap_err().kind(),
            io::ErrorKind::InvalidInput,
        );
    }

    #[test]
    fn test_u2fhid_encode_single() {
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        let mut data = [0u8; 8000];
        let len = codec.encode(b"\x01\x02\x03\x04", &mut data[..]).unwrap();
        assert_eq!(len, 64);
        let mut expect = [0u8; 64];
        expect[..11].copy_from_slice(b"\xEE\xEE\xEE\xEE\x55\x00\x04\x01\x02\x03\x04");
        assert_eq!(&data[..len], &expect[..]);
    }

    #[test]
    fn test_u2fhid_encode_multi() {
        let payload: Vec<u8> = (0..65u8).collect();
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        let mut data = [0u8; 8000];
        let len = codec.encode(&payload[..], &mut data[..]).unwrap();
        assert_eq!(len, 128);
        let mut expect = [0u8; 128];
        expect[..7].copy_from_slice(b"\xEE\xEE\xEE\xEE\x55\x00\x41");
        expect[7..64].copy_from_slice(&payload[..57]);
        expect[64..69].copy_from_slice(b"\xEE\xEE\xEE\xEE\x00");
        expect[69..77].copy_from_slice(&payload[57..]);
        assert_eq!(&data[..len], &expect[..]);
    }

    #[test]
    fn test_u2fhid_encode_last_continuation_frame() {
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        for len in [7551, 7609] {
            let payload = vec![0x42; len];
            let mut buf = [0; MAX_LEN];
            assert_eq!(codec.encode(&payload, &mut buf).unwrap(), MAX_LEN);
            assert_eq!(buf[128 * 64 + 4], 127);
            assert_eq!(codec.decode(&buf).unwrap().unwrap(), payload);
        }
    }

    #[test]
    fn test_u2fhid_decode_single() {
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        let mut raw = [0u8; 64];
        raw[..11].copy_from_slice(b"\xEE\xEE\xEE\xEE\x55\x00\x04\x01\x02\x03\x04");
        let data = codec.decode(&raw[..]).unwrap().unwrap();
        assert_eq!(&data[..], b"\x01\x02\x03\x04");
    }

    #[test]
    fn test_u2fhid_decode_multi() {
        let payload: Vec<u8> = (0..65u8).collect();
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        let mut raw = [0u8; 128];
        raw[..7].copy_from_slice(b"\xEE\xEE\xEE\xEE\x55\x00\x41");
        raw[7..64].copy_from_slice(&payload[..57]);
        raw[64..69].copy_from_slice(b"\xEE\xEE\xEE\xEE\x00");
        raw[69..77].copy_from_slice(&payload[57..]);
        let data = codec.decode(&raw[..]).unwrap().unwrap();
        assert_eq!(&data[..], &payload[..]);
    }

    #[test]
    fn test_u2fhid_decode_incomplete_reports() {
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        let mut raw = [0u8; 128];
        raw[..7].copy_from_slice(b"\xEE\xEE\xEE\xEE\x55\x00\x3A");
        raw[64..69].copy_from_slice(b"\xEE\xEE\xEE\xEE\x00");
        assert_eq!(codec.decode(&raw[..64]).unwrap(), None);
        for len in [0, 6, 63, 64 + 4, 127] {
            assert_eq!(
                codec.decode(&raw[..len]).unwrap_err().kind(),
                io::ErrorKind::UnexpectedEof,
            );
        }
    }

    #[test]
    fn test_u2fhid_decode_invalid_continuations() {
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        let mut raw = [0u8; 256];
        raw[..7].copy_from_slice(b"\xEE\xEE\xEE\xEE\x55\x00\x75");
        raw[64..69].copy_from_slice(b"\xEE\xEE\xEE\xEE\x00");
        raw[128..133].copy_from_slice(b"\xEE\xEE\xEE\xEE\x01");
        assert_eq!(codec.decode(&raw[..128]).unwrap(), None);
        assert_eq!(codec.decode(&raw).unwrap(), Some(vec![0; 117]));

        for (offset, bad_byte) in [(64, 0xFF), (68, 1), (128, 0xFF), (132, 0), (132, 128)] {
            let mut malformed = raw;
            malformed[offset] = bad_byte;
            assert_eq!(
                codec.decode(&malformed).unwrap_err().kind(),
                io::ErrorKind::InvalidData,
            );
        }
    }

    #[test]
    fn test_u2fhid_decode_oversized_announcement() {
        let codec = U2fHid::with_cid(0xEEEEEEEE, 0x55);
        let mut raw = [0u8; 64];
        raw[..7].copy_from_slice(b"\xEE\xEE\xEE\xEE\x55\x1D\xBA");
        assert_eq!(
            codec.decode(&raw).unwrap_err().kind(),
            io::ErrorKind::InvalidData,
        );
    }
}
