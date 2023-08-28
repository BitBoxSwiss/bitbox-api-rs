use crate::runtime::Runtime;

use crate::error::Error;
use crate::pb::{self, request::Request, response::Response};
use crate::Keypath;
use crate::PairedBitBox;

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
    feature = "serde",
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

impl<R: Runtime> PairedBitBox<R> {
    async fn sign_antiklepto(
        &self,
        request: pb::eth_request::Request,
        host_nonce: [u8; 32],
    ) -> Result<[u8; 65], Error> {
        match self.query_proto_eth(request).await? {
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
                        crate::antiklepto::verify_ecdsa(&host_nonce, &commitment, &signature)?;
                        signature.try_into().map_err(|_| Error::UnexpectedResponse)
                    }
                    _ => Err(Error::UnexpectedResponse),
                }
            }
            _ => Err(Error::UnexpectedResponse),
        }
    }

    // Signs an Ethereum transaction. It returns a 65 byte signature (R, S, and 1 byte recID).  The
    // `tx` param can be constructed manually or parsed from a raw transaction using
    // `raw_tx_slice.try_into()` (`rlp` feature required).
    pub async fn eth_sign_transaction(
        &self,
        chain_id: u64,
        keypath: &Keypath,
        tx: &Transaction,
    ) -> Result<[u8; 65], Error> {
        let host_nonce = crate::antiklepto::gen_host_nonce()?;
        let request = pb::eth_request::Request::Sign(pb::EthSignRequest {
            coin: 0,
            keypath: keypath.to_vec(),
            nonce: tx.nonce.clone(),
            gas_price: tx.gas_price.clone(),
            gas_limit: tx.gas_limit.clone(),
            recipient: tx.recipient.to_vec(),
            value: tx.value.clone(),
            data: tx.data.clone(),
            host_nonce_commitment: Some(pb::AntiKleptoHostNonceCommitment {
                commitment: crate::antiklepto::host_commit(&host_nonce).to_vec(),
            }),
            chain_id,
        });
        self.sign_antiklepto(request, host_nonce).await
    }

    // Signs an Ethereum message. The provided msg will be prefixed with "\x19Ethereum message\n" +
    // len(msg) in the hardware, e.g. "\x19Ethereum\n5hello" (yes, the len prefix is the ascii
    // representation with no fixed size or delimiter, WTF).  It returns a 65 byte signature (R, S,
    // and 1 byte recID). 27 is added to the recID to denote an uncompressed pubkey.
    pub async fn eth_sign_message(
        &self,
        chain_id: u64,
        keypath: &Keypath,
        msg: &[u8],
    ) -> Result<[u8; 65], Error> {
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
        let mut signature = self.sign_antiklepto(request, host_nonce).await?;
        // 27 is the magic constant to add to the recoverable ID to denote an
        // uncompressed pubkey.
        signature[64] += 27;
        Ok(signature)
    }
}
