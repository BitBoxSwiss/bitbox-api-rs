// SPDX-License-Identifier: Apache-2.0

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
}
