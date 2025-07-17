use crate::runtime::Runtime;

use crate::error::Error;
use crate::pb::{
    self,
    eth_sign_typed_message_request::{DataType, Member, MemberType, StructType},
    eth_typed_message_value_response::RootObject,
    request::Request,
    response::Response,
};
use crate::Keypath;
use crate::PairedBitBox;

use std::collections::HashMap;
use std::str::FromStr;

use num_bigint::{BigInt, BigUint};
//use num_traits::ToPrimitive;
use serde_json::Value;

impl<R: Runtime> PairedBitBox<R> {
    async fn query_proto_eth(
        &self,
        request: pb::eth_request::Request,
    ) -> Result<pb::eth_response::Response, Error> {
        match self
            .query_proto(Request::Eth(pb::EthRequest {
                request: Some(request),
            }))
            .await?
        {
            Response::Eth(pb::EthResponse {
                response: Some(response),
            }) => Ok(response),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Does this device support ETH functionality? Currently this means BitBox02 Multi.
    pub fn eth_supported(&self) -> bool {
        self.is_multi_edition()
    }

    /// Query the device for an xpub.
    pub async fn eth_xpub(&self, keypath: &Keypath) -> Result<String, Error> {
        match self
            .query_proto_eth(pb::eth_request::Request::Pub(pb::EthPubRequest {
                keypath: keypath.to_vec(),
                coin: 0,
                output_type: pb::eth_pub_request::OutputType::Xpub as _,
                display: false,
                contract_address: vec![],
                chain_id: 0,
            }))
            .await?
        {
            pb::eth_response::Response::Pub(pb::PubResponse { r#pub }) => Ok(r#pub),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Query the device for an Ethereum address.
    pub async fn eth_address(
        &self,
        chain_id: u64,
        keypath: &Keypath,
        display: bool,
    ) -> Result<String, Error> {
        match self
            .query_proto_eth(pb::eth_request::Request::Pub(pb::EthPubRequest {
                keypath: keypath.to_vec(),
                coin: 0,
                output_type: pb::eth_pub_request::OutputType::Address as _,
                display,
                contract_address: vec![],
                chain_id,
            }))
            .await?
        {
            pb::eth_response::Response::Pub(pb::PubResponse { r#pub }) => Ok(r#pub),
            _ => Err(Error::UnexpectedResponse),
        }
    }
}

#[cfg_attr(
    feature = "wasm",
    derive(serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Transaction {
    /// Nonce must be big-endian encoded, no trailing zeroes.
    pub nonce: Vec<u8>,
    /// Gas price must be big-endian encoded, no trailing zeroes.
    pub gas_price: Vec<u8>,
    /// Gas limit must be big-endian encoded, no trailing zeroes.
    pub gas_limit: Vec<u8>,
    pub recipient: [u8; 20],
    /// Value must be big-endian encoded, no trailing zeroes.
    pub value: Vec<u8>,
    pub data: Vec<u8>,
}

#[cfg_attr(
    feature = "wasm",
    derive(serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct EIP1559Transaction {
    pub chain_id: u64,
    /// Nonce must be big-endian encoded, no trailing zeroes.
    pub nonce: Vec<u8>,
    /// Max priority fee must be big-endian encoded, no trailing zeroes.
    pub max_priority_fee_per_gas: Vec<u8>,
    /// max fee must be big-endian encoded, no trailing zeroes.
    pub max_fee_per_gas: Vec<u8>,
    /// Gas limit must be big-endian encoded, no trailing zeroes.
    pub gas_limit: Vec<u8>,
    pub recipient: [u8; 20],
    /// Value must be big-endian encoded, no trailing zeroes.
    pub value: Vec<u8>,
    pub data: Vec<u8>,
}

/// Identifies the case of the recipient address given as hexadecimal string.
/// This function exists as a convenience to help clients to determine the case of the
/// recipient address.
pub fn eth_identify_case(recipient_address: &str) -> pb::EthAddressCase {
    if recipient_address
        .chars()
        .all(|c| !c.is_ascii_alphabetic() || c.is_ascii_uppercase())
    {
        pb::EthAddressCase::Upper
    } else if recipient_address
        .chars()
        .all(|c| !c.is_ascii_alphabetic() || c.is_ascii_lowercase())
    {
        pb::EthAddressCase::Lower
    } else {
        pb::EthAddressCase::Mixed
    }
}

#[cfg(feature = "rlp")]
impl TryFrom<&[u8]> for Transaction {
    type Error = ();

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let [nonce, gas_price, gas_limit, recipient, value, data, _, _, _]: [Vec<u8>; 9] =
            rlp::decode_list(value).try_into().map_err(|_| ())?;
        Ok(Transaction {
            nonce,
            gas_price,
            gas_limit,
            recipient: recipient.try_into().map_err(|_| ())?,
            value,
            data,
        })
    }
}

#[cfg(feature = "rlp")]
impl TryFrom<&[u8]> for EIP1559Transaction {
    type Error = ();

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let [mut chain_id_vec, nonce, max_priority_fee_per_gas, max_fee_per_gas, gas_limit, recipient, value, data, _, _, _]: [Vec<u8>; 11] =
            rlp::decode_list(value).try_into().map_err(|_| ())?;
        while chain_id_vec.len() < 8 {
            chain_id_vec.insert(0, 0);
        }
        let chain_id = u64::from_be_bytes(chain_id_vec.try_into().map_err(|_| ())?);
        Ok(EIP1559Transaction {
            chain_id,
            nonce,
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas_limit,
            recipient: recipient.try_into().map_err(|_| ())?,
            value,
            data,
        })
    }
}

#[derive(Debug, PartialEq, serde::Deserialize)]
struct Eip712TypeMember {
    name: String,
    r#type: String,
}

#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Eip712Message {
    types: HashMap<String, Vec<Eip712TypeMember>>,
    primary_type: String,
    domain: HashMap<String, Value>,
    message: HashMap<String, Value>,
}

fn parse_type(
    typ: &str,
    types: &HashMap<String, Vec<Eip712TypeMember>>,
) -> Result<MemberType, String> {
    if typ.ends_with(']') {
        let index = typ
            .rfind('[')
            .ok_or(format!("Invalid type format: {typ}"))?;
        let (rest, size) = (&typ[..index], &typ[index + 1..typ.len() - 1]);
        let size_int = if !size.is_empty() {
            u32::from_str(size).map_err(|e| format!("Error parsing size: {e}"))?
        } else {
            0
        };
        let array_type = Box::new(parse_type(rest, types)?);
        Ok(MemberType {
            r#type: DataType::Array.into(),
            size: size_int,
            struct_name: String::new(),
            array_type: Some(array_type),
        })
    } else if let Some(size) = typ.strip_prefix("bytes") {
        let size_int = if !size.is_empty() {
            u32::from_str(size).map_err(|e| format!("Error parsing size: {e}"))?
        } else {
            0
        };
        Ok(MemberType {
            r#type: DataType::Bytes.into(),
            size: size_int,
            struct_name: String::new(),
            array_type: None,
        })
    } else if let Some(size) = typ.strip_prefix("uint") {
        if size.is_empty() {
            return Err("uint must be sized".to_string());
        }
        let size_int = u32::from_str(size).map_err(|e| format!("Error parsing size: {e}"))? / 8;
        Ok(MemberType {
            r#type: DataType::Uint.into(),
            size: size_int,
            struct_name: String::new(),
            array_type: None,
        })
    } else if let Some(size) = typ.strip_prefix("int") {
        if size.is_empty() {
            return Err("int must be sized".to_string());
        }
        let size_int = u32::from_str(size).map_err(|e| format!("Error parsing size: {e}"))? / 8;
        Ok(MemberType {
            r#type: DataType::Int.into(),
            size: size_int,
            struct_name: String::new(),
            array_type: None,
        })
    } else if typ == "bool" {
        Ok(MemberType {
            r#type: DataType::Bool.into(),
            size: 0,
            struct_name: String::new(),
            array_type: None,
        })
    } else if typ == "address" {
        Ok(MemberType {
            r#type: DataType::Address.into(),
            size: 0,
            struct_name: String::new(),
            array_type: None,
        })
    } else if typ == "string" {
        Ok(MemberType {
            r#type: DataType::String.into(),
            size: 0,
            struct_name: String::new(),
            array_type: None,
        })
    } else if types.contains_key(typ) {
        Ok(MemberType {
            r#type: DataType::Struct.into(),
            size: 0,
            struct_name: typ.to_string(),
            array_type: None,
        })
    } else {
        Err(format!("Can't recognize type: {typ}"))
    }
}

fn encode_value(typ: &MemberType, value: &Value) -> Result<Vec<u8>, String> {
    match DataType::try_from(typ.r#type).unwrap() {
        DataType::Bytes => {
            if let Value::String(v) = value {
                if v.starts_with("0x") || v.starts_with("0X") {
                    hex::decode(&v[2..]).map_err(|e| e.to_string())
                } else {
                    Ok(v.as_bytes().to_vec())
                }
            } else {
                Err("Expected a string for bytes type".to_string())
            }
        }
        DataType::Uint => match value {
            Value::String(v) => {
                if v.starts_with("0x") || v.starts_with("0X") {
                    Ok(BigUint::parse_bytes(&v.as_bytes()[2..], 16)
                        .ok_or(format!("could not parse {v} as hex"))?
                        .to_bytes_be())
                } else {
                    Ok(BigUint::from_str(v)
                        .map_err(|e| e.to_string())?
                        .to_bytes_be())
                }
            }
            Value::Number(n) => {
                if let Some(v) = n.as_f64() {
                    let v64: u64 = v as _;
                    if (v64 as f64) != v {
                        Err("Number is not an uint".to_string())
                    } else {
                        Ok(BigUint::from(v64).to_bytes_be())
                    }
                } else {
                    Err("Number is not an uint".to_string())
                }
            }
            _ => Err("Wrong type for uint".to_string()),
        },
        DataType::Int => match value {
            Value::String(v) => Ok(BigInt::from_str(v)
                .map_err(|e| e.to_string())?
                .to_signed_bytes_be()),
            Value::Number(n) => {
                if let Some(v) = n.as_f64() {
                    let v64: i64 = v as _;
                    if (v64 as f64) != v {
                        Err("Number is not an int".to_string())
                    } else {
                        let mut bytes = BigInt::from(v64).to_signed_bytes_be();
                        // Drop leading zero. There can be at most one.
                        if let [0, ..] = bytes.as_slice() {
                            bytes.remove(0);
                        }
                        Ok(bytes)
                    }
                } else {
                    Err("Number is not an int".to_string())
                }
            }
            _ => Err("Wrong type for int".to_string()),
        },
        DataType::Bool => {
            if value.as_bool().ok_or("Expected a boolean value")? {
                Ok(vec![1])
            } else {
                Ok(vec![0])
            }
        }
        DataType::Address | DataType::String => {
            if let Value::String(v) = value {
                Ok(v.as_bytes().to_vec())
            } else {
                Err("Expected a string value".to_string())
            }
        }
        DataType::Array => {
            if let Value::Array(arr) = value {
                let size = arr.len() as u32;
                Ok(size.to_be_bytes().to_vec())
            } else {
                Err("Expected an array".to_string())
            }
        }
        _ => Err("looked up value must not be a map or array".to_string()),
    }
}

fn get_value(
    what: &pb::EthTypedMessageValueResponse,
    msg: &Eip712Message,
) -> Result<Vec<u8>, String> {
    enum Either<'a> {
        HashMap(&'a HashMap<String, Value>),
        JsonValue(Value),
    }
    impl Either<'_> {
        fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Either::HashMap(map) => map.get(key),
                Either::JsonValue(Value::Object(map)) => map.get(key),
                _ => None,
            }
        }
    }

    let (mut value, mut typ): (Either, MemberType) =
        match RootObject::try_from(what.root_object).unwrap() {
            RootObject::Unknown => return Err("unkown root object".into()),
            RootObject::Domain => (
                Either::HashMap(&msg.domain),
                parse_type("EIP712Domain", &msg.types)?,
            ),
            RootObject::Message => (
                Either::HashMap(&msg.message),
                parse_type(&msg.primary_type, &msg.types)?,
            ),
        };
    for element in what.path.iter() {
        match DataType::try_from(typ.r#type).unwrap() {
            DataType::Struct => {
                let struct_member: &Eip712TypeMember = msg
                    .types
                    .get(&typ.struct_name)
                    .ok_or(format!(
                        "could not lookup type of name: {}",
                        &typ.struct_name
                    ))?
                    .get(*element as usize)
                    .ok_or(format!(
                        "could not lookup member #{} of type: {}",
                        *element, &typ.struct_name
                    ))?;
                value = Either::JsonValue(
                    value
                        .get(&struct_member.name)
                        .ok_or(format!("could not lookup: {}", struct_member.name.as_str()))?
                        .clone(),
                );
                typ = parse_type(&struct_member.r#type, &msg.types)?;
            }
            DataType::Array => {
                if let Either::JsonValue(Value::Array(list)) = value {
                    value = Either::JsonValue(
                        list.get(*element as usize)
                            .ok_or(format!("could not lookup array index: {}", *element))?
                            .clone(),
                    );
                    typ = *typ.array_type.unwrap();
                }
            }
            _ => return Err("path element does not point to struct or array".into()),
        }
    }
    if let Either::JsonValue(value) = &value {
        encode_value(&typ, value)
    } else {
        Err("path points to struct or array; value expected".to_string())
    }
}

impl<R: Runtime> PairedBitBox<R> {
    async fn handle_antiklepto(
        &self,
        response: &pb::eth_response::Response,
        host_nonce: [u8; 32],
    ) -> Result<[u8; 65], Error> {
        match response {
            pb::eth_response::Response::AntikleptoSignerCommitment(
                pb::AntiKleptoSignerCommitment { commitment },
            ) => {
                match self
                    .query_proto_eth(pb::eth_request::Request::AntikleptoSignature(
                        pb::AntiKleptoSignatureRequest {
                            host_nonce: host_nonce.to_vec(),
                        },
                    ))
                    .await?
                {
                    pb::eth_response::Response::Sign(pb::EthSignResponse { signature }) => {
                        crate::antiklepto::verify_ecdsa(&host_nonce, commitment, &signature)?;
                        signature.try_into().map_err(|_| Error::UnexpectedResponse)
                    }
                    _ => Err(Error::UnexpectedResponse),
                }
            }
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Signs an Ethereum transaction. It returns a 65 byte signature (R, S, and 1 byte recID).  The
    /// `tx` param can be constructed manually or parsed from a raw transaction using
    /// `raw_tx_slice.try_into()` (`rlp` feature required).
    pub async fn eth_sign_transaction(
        &self,
        chain_id: u64,
        keypath: &Keypath,
        tx: &Transaction,
        address_case: Option<pb::EthAddressCase>,
    ) -> Result<[u8; 65], Error> {
        // passing chainID instead of coin only since v9.10.0
        self.validate_version(">=9.10.0")?;

        let host_nonce = crate::antiklepto::gen_host_nonce()?;
        let request = pb::eth_request::Request::Sign(pb::EthSignRequest {
            coin: 0,
            keypath: keypath.to_vec(),
            nonce: crate::util::remove_leading_zeroes(&tx.nonce),
            gas_price: crate::util::remove_leading_zeroes(&tx.gas_price),
            gas_limit: crate::util::remove_leading_zeroes(&tx.gas_limit),
            recipient: tx.recipient.to_vec(),
            value: crate::util::remove_leading_zeroes(&tx.value),
            data: tx.data.clone(),
            host_nonce_commitment: Some(pb::AntiKleptoHostNonceCommitment {
                commitment: crate::antiklepto::host_commit(&host_nonce).to_vec(),
            }),
            chain_id,
            address_case: address_case.unwrap_or(pb::EthAddressCase::Mixed).into(),
        });
        let response = self.query_proto_eth(request).await?;
        self.handle_antiklepto(&response, host_nonce).await
    }

    /// Signs an Ethereum type 2 transaction according to EIP 1559. It returns a 65 byte signature (R, S, and 1 byte recID).
    /// The `tx` param can be constructed manually or parsed from a raw transaction using
    /// `raw_tx_slice.try_into()` (`rlp` feature required).
    pub async fn eth_sign_1559_transaction(
        &self,
        keypath: &Keypath,
        tx: &EIP1559Transaction,
        address_case: Option<pb::EthAddressCase>,
    ) -> Result<[u8; 65], Error> {
        // EIP1559 is suported from v9.16.0
        self.validate_version(">=9.16.0")?;

        let host_nonce = crate::antiklepto::gen_host_nonce()?;
        let request = pb::eth_request::Request::SignEip1559(pb::EthSignEip1559Request {
            chain_id: tx.chain_id,
            keypath: keypath.to_vec(),
            nonce: crate::util::remove_leading_zeroes(&tx.nonce),
            max_priority_fee_per_gas: crate::util::remove_leading_zeroes(
                &tx.max_priority_fee_per_gas,
            ),
            max_fee_per_gas: crate::util::remove_leading_zeroes(&tx.max_fee_per_gas),
            gas_limit: crate::util::remove_leading_zeroes(&tx.gas_limit),
            recipient: tx.recipient.to_vec(),
            value: crate::util::remove_leading_zeroes(&tx.value),
            data: tx.data.clone(),
            host_nonce_commitment: Some(pb::AntiKleptoHostNonceCommitment {
                commitment: crate::antiklepto::host_commit(&host_nonce).to_vec(),
            }),
            address_case: address_case.unwrap_or(pb::EthAddressCase::Mixed).into(),
        });
        let response = self.query_proto_eth(request).await?;
        self.handle_antiklepto(&response, host_nonce).await
    }

    /// Signs an Ethereum message. The provided msg will be prefixed with "\x19Ethereum message\n" +
    /// len(msg) in the hardware, e.g. "\x19Ethereum\n5hello" (yes, the len prefix is the ascii
    /// representation with no fixed size or delimiter).  It returns a 65 byte signature (R, S, and
    /// 1 byte recID). 27 is added to the recID to denote an uncompressed pubkey.
    pub async fn eth_sign_message(
        &self,
        chain_id: u64,
        keypath: &Keypath,
        msg: &[u8],
    ) -> Result<[u8; 65], Error> {
        // passing chainID instead of coin only since v9.10.0
        self.validate_version(">=9.10.0")?;

        let host_nonce = crate::antiklepto::gen_host_nonce()?;
        let request = pb::eth_request::Request::SignMsg(pb::EthSignMessageRequest {
            coin: 0,
            keypath: keypath.to_vec(),
            msg: msg.to_vec(),
            host_nonce_commitment: Some(pb::AntiKleptoHostNonceCommitment {
                commitment: crate::antiklepto::host_commit(&host_nonce).to_vec(),
            }),
            chain_id,
        });
        let response = self.query_proto_eth(request).await?;
        let mut signature = self.handle_antiklepto(&response, host_nonce).await?;
        // 27 is the magic constant to add to the recoverable ID to denote an uncompressed pubkey.
        signature[64] += 27;
        Ok(signature)
    }

    /// Signs an Ethereum EIP-712 typed message. It returns a 65 byte signature (R, S, and 1 byte
    /// recID). 27 is added to the recID to denote an uncompressed pubkey.
    pub async fn eth_sign_typed_message(
        &self,
        chain_id: u64,
        keypath: &Keypath,
        json_msg: &str,
    ) -> Result<[u8; 65], Error> {
        self.validate_version(">=9.12.0")?;

        let msg: Eip712Message = serde_json::from_str(json_msg)
            .map_err(|_| Error::EthTypedMessage("Could not parse EIP-712 JSON message".into()))?;

        let parsed_types: Vec<StructType> = msg
            .types
            .iter()
            .map(|(name, members)| {
                Ok(StructType {
                    name: name.clone(),
                    members: members
                        .iter()
                        .map(|member| {
                            Ok(Member {
                                name: member.name.clone(),
                                r#type: Some(parse_type(&member.r#type, &msg.types)?),
                            })
                        })
                        .collect::<Result<Vec<Member>, String>>()?,
                })
            })
            .collect::<Result<Vec<StructType>, String>>()
            .map_err(Error::EthTypedMessage)?;

        let host_nonce = crate::antiklepto::gen_host_nonce()?;

        let mut response = self
            .query_proto_eth(pb::eth_request::Request::SignTypedMsg(
                pb::EthSignTypedMessageRequest {
                    chain_id,
                    keypath: keypath.to_vec(),
                    types: parsed_types,
                    primary_type: msg.primary_type.clone(),
                    host_nonce_commitment: Some(pb::AntiKleptoHostNonceCommitment {
                        commitment: crate::antiklepto::host_commit(&host_nonce).to_vec(),
                    }),
                },
            ))
            .await?;
        while let pb::eth_response::Response::TypedMsgValue(typed_msg_value) = &response {
            let value = get_value(typed_msg_value, &msg).map_err(Error::EthTypedMessage)?;
            response = self
                .query_proto_eth(pb::eth_request::Request::TypedMsgValue(
                    pb::EthTypedMessageValueRequest { value },
                ))
                .await?;
        }
        let mut signature = self.handle_antiklepto(&response, host_nonce).await?;
        // 27 is the magic constant to add to the recoverable ID to denote an uncompressed pubkey.
        signature[64] += 27;
        Ok(signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EIP712_MSG: &str = r#"
        {
    "types": {
        "EIP712Domain": [
            { "name": "name", "type": "string" },
            { "name": "version", "type": "string" },
            { "name": "chainId", "type": "uint256" },
            { "name": "verifyingContract", "type": "address" }
        ],
        "Attachment": [
            { "name": "contents", "type": "string" }
        ],
        "Person": [
            { "name": "name", "type": "string" },
            { "name": "wallet", "type": "address" },
            { "name": "age", "type": "uint8" }
        ],
        "Mail": [
            { "name": "from", "type": "Person" },
            { "name": "to", "type": "Person" },
            { "name": "contents", "type": "string" },
            { "name": "attachments", "type": "Attachment[]" }
        ]
    },
    "primaryType": "Mail",
    "domain": {
        "name": "Ether Mail",
        "version": "1",
        "chainId": 1,
        "verifyingContract": "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"
    },
    "message": {
        "from": {
            "name": "Cow",
            "wallet": "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826",
            "age": 20
        },
        "to": {
            "name": "Bob",
            "wallet": "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB",
            "age": "0x1e"
        },
        "contents": "Hello, Bob!",
        "attachments": [{ "contents": "attachment1" }, { "contents": "attachment2" }]
    }
}
    "#;

    fn map_from(elements: &[(&str, Value)]) -> Value {
        let mut m = serde_json::Map::<String, Value>::new();
        for (k, v) in elements.iter().cloned() {
            m.insert(k.into(), v);
        }
        m.into()
    }

    #[test]
    fn test_deserialize_eip713_message() {
        let msg: Eip712Message = serde_json::from_str(EIP712_MSG).unwrap();
        assert_eq!(
            msg,
            Eip712Message {
                types: HashMap::from([
                    (
                        "EIP712Domain".into(),
                        vec![
                            Eip712TypeMember {
                                name: "name".into(),
                                r#type: "string".into()
                            },
                            Eip712TypeMember {
                                name: "version".into(),
                                r#type: "string".into()
                            },
                            Eip712TypeMember {
                                name: "chainId".into(),
                                r#type: "uint256".into()
                            },
                            Eip712TypeMember {
                                name: "verifyingContract".into(),
                                r#type: "address".into()
                            },
                        ]
                    ),
                    (
                        "Attachment".into(),
                        vec![Eip712TypeMember {
                            name: "contents".into(),
                            r#type: "string".into()
                        },]
                    ),
                    (
                        "Person".into(),
                        vec![
                            Eip712TypeMember {
                                name: "name".into(),
                                r#type: "string".into()
                            },
                            Eip712TypeMember {
                                name: "wallet".into(),
                                r#type: "address".into()
                            },
                            Eip712TypeMember {
                                name: "age".into(),
                                r#type: "uint8".into()
                            },
                        ]
                    ),
                    (
                        "Mail".into(),
                        vec![
                            Eip712TypeMember {
                                name: "from".into(),
                                r#type: "Person".into()
                            },
                            Eip712TypeMember {
                                name: "to".into(),
                                r#type: "Person".into()
                            },
                            Eip712TypeMember {
                                name: "contents".into(),
                                r#type: "string".into()
                            },
                            Eip712TypeMember {
                                name: "attachments".into(),
                                r#type: "Attachment[]".into()
                            },
                        ]
                    ),
                ]),
                primary_type: "Mail".into(),
                domain: HashMap::from([
                    ("name".into(), "Ether Mail".into()),
                    ("version".into(), "1".into()),
                    ("chainId".into(), 1.into()),
                    (
                        "verifyingContract".into(),
                        "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC".into()
                    ),
                ]),
                message: HashMap::from([
                    (
                        "from".into(),
                        map_from(&[
                            ("name", "Cow".into()),
                            (
                                "wallet",
                                "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".into()
                            ),
                            ("age", 20.into())
                        ]),
                    ),
                    (
                        "to".into(),
                        map_from(&[
                            ("name", "Bob".into()),
                            (
                                "wallet",
                                "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB".into()
                            ),
                            ("age", "0x1e".into())
                        ]),
                    ),
                    ("contents".into(), "Hello, Bob!".into()),
                    (
                        "attachments".into(),
                        Value::Array(vec![
                            map_from(&[("contents", "attachment1".into())]),
                            map_from(&[("contents", "attachment2".into())])
                        ])
                    ),
                ]),
            }
        );
    }

    fn parse_type_no_err(typ: &str, types: &HashMap<String, Vec<Eip712TypeMember>>) -> MemberType {
        parse_type(typ, types).unwrap()
    }

    #[test]
    fn test_parse_type() {
        assert_eq!(
            MemberType {
                r#type: DataType::String.into(),
                ..Default::default()
            },
            parse_type_no_err("string", &HashMap::new())
        );

        // Bytes.
        assert_eq!(
            MemberType {
                r#type: DataType::Bytes.into(),
                ..Default::default()
            },
            parse_type_no_err("bytes", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Bytes.into(),
                size: 1,
                ..Default::default()
            },
            parse_type_no_err("bytes1", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Bytes.into(),
                size: 10,
                ..Default::default()
            },
            parse_type_no_err("bytes10", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Bytes.into(),
                size: 32,
                ..Default::default()
            },
            parse_type_no_err("bytes32", &HashMap::new())
        );

        assert_eq!(
            MemberType {
                r#type: DataType::Bool.into(),
                ..Default::default()
            },
            parse_type_no_err("bool", &HashMap::new())
        );

        assert_eq!(
            MemberType {
                r#type: DataType::Address.into(),
                ..Default::default()
            },
            parse_type_no_err("address", &HashMap::new())
        );

        // Uints.
        assert_eq!(
            MemberType {
                r#type: DataType::Uint.into(),
                size: 1,
                ..Default::default()
            },
            parse_type_no_err("uint8", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Uint.into(),
                size: 2,
                ..Default::default()
            },
            parse_type_no_err("uint16", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Uint.into(),
                size: 32,
                ..Default::default()
            },
            parse_type_no_err("uint256", &HashMap::new())
        );
        assert!(parse_type("uint", &HashMap::new()).is_err());
        assert!(parse_type("uintfoo", &HashMap::new()).is_err());

        // Ints.
        assert_eq!(
            MemberType {
                r#type: DataType::Int.into(),
                size: 1,
                ..Default::default()
            },
            parse_type_no_err("int8", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Int.into(),
                size: 2,
                ..Default::default()
            },
            parse_type_no_err("int16", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Int.into(),
                size: 32,
                ..Default::default()
            },
            parse_type_no_err("int256", &HashMap::new())
        );
        assert!(parse_type("int", &HashMap::new()).is_err());
        assert!(parse_type("intfoo", &HashMap::new()).is_err());

        // Arrays.
        assert_eq!(
            MemberType {
                r#type: DataType::Array.into(),
                array_type: Some(Box::new(MemberType {
                    r#type: DataType::String.into(),
                    ..Default::default()
                })),
                ..Default::default()
            },
            parse_type_no_err("string[]", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Array.into(),
                size: 521,
                array_type: Some(Box::new(MemberType {
                    r#type: DataType::String.into(),
                    ..Default::default()
                })),
                ..Default::default()
            },
            parse_type_no_err("string[521]", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Array.into(),
                size: 521,
                array_type: Some(Box::new(MemberType {
                    r#type: DataType::Uint.into(),
                    size: 4,
                    ..Default::default()
                })),
                ..Default::default()
            },
            parse_type_no_err("uint32[521]", &HashMap::new())
        );
        assert_eq!(
            MemberType {
                r#type: DataType::Array.into(),
                array_type: Some(Box::new(MemberType {
                    r#type: DataType::Array.into(),
                    size: 521,
                    array_type: Some(Box::new(MemberType {
                        r#type: DataType::Uint.into(),
                        size: 4,
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            },
            parse_type_no_err("uint32[521][]", &HashMap::new())
        );

        // Structs
        assert!(parse_type("Unknown", &HashMap::new()).is_err());

        assert_eq!(
            MemberType {
                r#type: DataType::Struct.into(),
                struct_name: "Person".to_string(),
                ..Default::default()
            },
            parse_type_no_err(
                "Person",
                &HashMap::from([("Person".to_string(), Vec::new())])
            )
        );
    }

    #[test]
    fn test_encode_value() {
        let encoded =
            encode_value(&parse_type_no_err("bytes", &HashMap::new()), &"foo".into()).unwrap();
        assert_eq!(b"foo".to_vec(), encoded);

        let encoded = encode_value(
            &parse_type_no_err("bytes3", &HashMap::new()),
            &"0xaabbcc".into(),
        )
        .unwrap();
        assert_eq!(vec![0xaa, 0xbb, 0xcc], encoded);

        let encoded = encode_value(
            &parse_type_no_err("uint64", &HashMap::new()),
            &2983742332.0.into(),
        )
        .unwrap();
        assert_eq!(vec![0xb1, 0xd8, 0x4b, 0x7c], encoded);

        let encoded = encode_value(
            &parse_type_no_err("uint64", &HashMap::new()),
            &"0xb1d84b7c".into(),
        )
        .unwrap();
        assert_eq!(vec![0xb1, 0xd8, 0x4b, 0x7c], encoded);

        let encoded =
            encode_value(&parse_type_no_err("uint64", &HashMap::new()), &"0x1".into()).unwrap();
        assert_eq!(vec![0x01], encoded);

        let encoded = encode_value(
            &parse_type_no_err("uint64", &HashMap::new()),
            &"0x0001".into(),
        )
        .unwrap();
        assert_eq!(vec![0x01], encoded);

        assert!(encode_value(
            &parse_type_no_err("uint64", &HashMap::new()),
            &"0xnot correct".into(),
        )
        .is_err());

        let encoded = encode_value(
            &parse_type_no_err("int64", &HashMap::new()),
            &2983742332.0.into(),
        )
        .unwrap();
        assert_eq!(vec![0xb1, 0xd8, 0x4b, 0x7c], encoded);

        let encoded = encode_value(
            &parse_type_no_err("int64", &HashMap::new()),
            &(-2983742332.0).into(),
        )
        .unwrap();
        assert_eq!(vec![0xff, 0x4e, 0x27, 0xb4, 0x84], encoded);

        let encoded =
            encode_value(&parse_type_no_err("string", &HashMap::new()), &"foo".into()).unwrap();
        assert_eq!(b"foo".to_vec(), encoded);

        let encoded = encode_value(
            &parse_type_no_err("address", &HashMap::new()),
            &"0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC".into(),
        )
        .unwrap();
        assert_eq!(
            b"0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC".to_vec(),
            encoded
        );

        let encoded =
            encode_value(&parse_type_no_err("bool", &HashMap::new()), &false.into()).unwrap();
        assert_eq!(vec![0], encoded);

        let encoded =
            encode_value(&parse_type_no_err("bool", &HashMap::new()), &true.into()).unwrap();
        assert_eq!(vec![1], encoded);

        // Array encodes its size.
        let encoded = encode_value(
            &parse_type_no_err("bool[]", &HashMap::new()),
            &Value::Array(vec![]),
        )
        .unwrap();
        assert_eq!(b"\x00\x00\x00\x00".to_vec(), encoded);

        let encoded = encode_value(
            &parse_type_no_err("uint8[]", &HashMap::new()),
            &Value::Array(vec![1.into(); 10]),
        )
        .unwrap();
        assert_eq!(b"\x00\x00\x00\x0a".to_vec(), encoded);

        let encoded = encode_value(
            &parse_type_no_err("uint8[]", &HashMap::new()),
            &Value::Array(vec![1.into(); 1000]),
        )
        .unwrap();
        assert_eq!(b"\x00\x00\x03\xe8".to_vec(), encoded);
    }

    #[test]
    fn test_get_value() {
        let msg: Eip712Message = serde_json::from_str(EIP712_MSG).unwrap();

        // DOMAIN

        // Can't lookup domain itself.
        assert!(get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Domain as _,
                path: vec![],
            },
            &msg,
        )
        .is_err());
        // Path points to nowhere.
        assert!(get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Domain as _,
                path: vec![0, 0],
            },
            &msg,
        )
        .is_err());

        // domain.name
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Domain as _,
                path: vec![0],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(value, b"Ether Mail".to_vec());

        // domain.version
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Domain as _,
                path: vec![1],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(value, b"1".to_vec());

        // domain.chainId
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Domain as _,
                path: vec![2],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(value, b"\x01".to_vec());

        // domain.verifyingContract
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Domain as _,
                path: vec![3],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(
            value,
            b"0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC".to_vec()
        );
        // No more members.
        assert!(get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Domain as _,
                path: vec![4],
            },
            &msg,
        )
        .is_err());

        // MESSAGE

        // message.from.name
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Message as _,
                path: vec![0, 0],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(value, b"Cow".to_vec());

        // message.from.wallet
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Message as _,
                path: vec![0, 1],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(
            value,
            b"0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_vec()
        );

        // message.to.wallet
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Message as _,
                path: vec![1, 1],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(
            value,
            b"0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB".to_vec()
        );

        // message.attachments.0.contents
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Message as _,
                path: vec![3, 0, 0],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(value, b"attachment1".to_vec());

        // message.attachments.1.contents
        let value = get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Message as _,
                path: vec![3, 1, 0],
            },
            &msg,
        )
        .unwrap();
        assert_eq!(value, b"attachment2".to_vec());

        // no more attachments
        assert!(get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Message as _,
                path: vec![3, 2, 0],
            },
            &msg,
        )
        .is_err());

        // only one field in attachment
        assert!(get_value(
            &pb::EthTypedMessageValueResponse {
                root_object: RootObject::Message as _,
                path: vec![3, 1, 1],
            },
            &msg,
        )
        .is_err());
    }

    #[test]
    fn test_eth_identify_case() {
        assert_eq!(
            eth_identify_case("0XF39FD6E51AAD88F6F4CE6AB8827279CFFFB92266"),
            pb::EthAddressCase::Upper
        );
        assert_eq!(
            eth_identify_case("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
            pb::EthAddressCase::Lower
        );
        assert_eq!(
            eth_identify_case("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            pb::EthAddressCase::Mixed
        );
    }
}
