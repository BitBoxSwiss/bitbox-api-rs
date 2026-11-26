// SPDX-License-Identifier: Apache-2.0

use bitcoin::bip32::Fingerprint;
use prost::Message;

use crate::error::Error;
use crate::pb::{self, request::Request, response::Response};

pub fn encode_request(request: Request) -> Vec<u8> {
    pb::Request {
        request: Some(request),
    }
    .encode_to_vec()
}

pub fn decode_response(bytes: &[u8]) -> Result<pb::Response, prost::DecodeError> {
    pb::Response::decode(bytes)
}

pub fn response_body(response: pb::Response) -> Result<Response, Error> {
    response.response.ok_or(Error::ProtobufDecode)
}

pub fn root_fingerprint_request() -> Request {
    Request::Fingerprint(pb::RootFingerprintRequest {})
}

pub fn device_info_request() -> Request {
    Request::DeviceInfo(pb::DeviceInfoRequest {})
}

pub fn show_mnemonic_request() -> Request {
    Request::ShowMnemonic(pb::ShowMnemonicRequest {})
}

pub fn set_device_name_request(name: String) -> Request {
    Request::DeviceName(pb::SetDeviceNameRequest { name })
}

pub fn set_password_request(entropy: &[u8; 32]) -> Request {
    Request::SetPassword(pb::SetPasswordRequest {
        entropy: entropy.to_vec(),
    })
}

pub fn create_backup_request(timestamp: u32, timezone_offset: i32) -> Request {
    Request::CreateBackup(pb::CreateBackupRequest {
        timestamp,
        timezone_offset,
    })
}

pub fn restore_from_mnemonic_request(timestamp: u32, timezone_offset: i32) -> Request {
    Request::RestoreFromMnemonic(pb::RestoreFromMnemonicRequest {
        timestamp,
        timezone_offset,
    })
}

pub fn reset_request() -> Request {
    Request::Reset(pb::ResetRequest {})
}

pub fn set_mnemonic_passphrase_enabled_request(enabled: bool) -> Request {
    Request::SetMnemonicPassphraseEnabled(pb::SetMnemonicPassphraseEnabledRequest { enabled })
}

pub fn parse_device_info(response: Response) -> Result<pb::DeviceInfoResponse, Error> {
    match response {
        Response::DeviceInfo(info) => Ok(info),
        _ => Err(Error::UnexpectedResponse),
    }
}

pub fn parse_root_fingerprint(response: Response) -> Result<Fingerprint, Error> {
    match response {
        Response::Fingerprint(info) => {
            let bytes: [u8; 4] = info
                .fingerprint
                .try_into()
                .map_err(|_| Error::UnexpectedResponse)?;
            Ok(Fingerprint::from(bytes))
        }
        _ => Err(Error::UnexpectedResponse),
    }
}

pub fn parse_pub(response: Response) -> Result<String, Error> {
    match response {
        Response::Pub(value) => Ok(value.r#pub),
        _ => Err(Error::UnexpectedResponse),
    }
}

pub fn parse_success(response: Response) -> Result<(), Error> {
    match response {
        Response::Success(_) => Ok(()),
        _ => Err(Error::UnexpectedResponse),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_error_without_losing_code_or_message() {
        let wire = b"\x12\x08\x08\xe7\x07\x12\x03new";
        assert_eq!(
            response_body(decode_response(wire).unwrap()).unwrap(),
            Response::Error(pb::Error {
                code: 999,
                message: "new".into(),
            }),
        );
        assert!(matches!(
            response_body(decode_response(&[]).unwrap()),
            Err(Error::ProtobufDecode),
        ));
        assert!(decode_response(&wire[..wire.len() - 1]).is_err());
    }

    #[test]
    fn encodes_management_requests() {
        let entropy_wire = [&[0x2a, 34, 0x0a, 32][..], &[0x42; 32]].concat();
        for (request, wire) in [
            (root_fingerprint_request(), &b"\xc2\x01\x00"[..]),
            (device_info_request(), &b"\x22\x00"[..]),
            (show_mnemonic_request(), &b"\x3a\x00"[..]),
            (
                set_device_name_request("unit".into()),
                &b"\x12\x06\x0a\x04unit"[..],
            ),
            (set_password_request(&[0x42; 32]), &entropy_wire[..]),
            (
                create_backup_request(123, 60),
                &b"\x32\x04\x08\x7b\x10\x3c"[..],
            ),
            (
                restore_from_mnemonic_request(123, 60),
                &b"\xb2\x01\x04\x08\x7b\x10\x3c"[..],
            ),
            (reset_request(), &b"\xaa\x01\x00"[..]),
            (
                set_mnemonic_passphrase_enabled_request(true),
                &b"\x72\x02\x08\x01"[..],
            ),
            (
                set_mnemonic_passphrase_enabled_request(false),
                &b"\x72\x00"[..],
            ),
        ] {
            assert_eq!(encode_request(request), wire);
        }
    }

    #[test]
    fn fingerprint_requires_exactly_four_bytes() {
        let parse = |bytes: &[u8]| {
            parse_root_fingerprint(Response::Fingerprint(pb::RootFingerprintResponse {
                fingerprint: bytes.to_vec(),
            }))
        };
        assert_eq!(parse(&[1, 2, 3, 4]).unwrap().to_bytes(), [1, 2, 3, 4]);
        for bytes in [&[1, 2, 3][..], &[1, 2, 3, 4, 5]] {
            assert!(matches!(parse(bytes), Err(Error::UnexpectedResponse)));
        }
        assert!(matches!(
            parse_root_fingerprint(Response::Success(pb::Success {})),
            Err(Error::UnexpectedResponse),
        ));
    }

    #[test]
    fn checks_response_variants() {
        let info = pb::DeviceInfoResponse {
            version: "9.29.0".into(),
            ..Default::default()
        };
        assert_eq!(
            parse_device_info(Response::DeviceInfo(info.clone())).unwrap(),
            info
        );
        let public = Response::Pub(pb::PubResponse {
            r#pub: "address".into(),
        });
        let success = Response::Success(pb::Success {});
        assert_eq!(parse_pub(public.clone()).unwrap(), "address");
        assert!(parse_success(success.clone()).is_ok());
        assert!(matches!(
            parse_success(public),
            Err(Error::UnexpectedResponse)
        ));
        assert!(matches!(
            parse_pub(success.clone()),
            Err(Error::UnexpectedResponse)
        ));
        assert!(matches!(
            parse_device_info(success),
            Err(Error::UnexpectedResponse)
        ));
    }
}
