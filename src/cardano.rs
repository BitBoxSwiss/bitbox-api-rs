use crate::runtime::Runtime;

use crate::error::Error;
use crate::pb::{self, request::Request, response::Response};
use crate::Keypath;
use crate::PairedBitBox;

#[cfg(feature = "wasm")]
pub(crate) fn serde_deserialize_network<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let network = pb::CardanoNetwork::deserialize(deserializer)?;
    Ok(network as i32)
}

#[cfg(feature = "wasm")]
pub(crate) fn serde_deserialize_drep_type<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let drep_type = pb::cardano_sign_transaction_request::certificate::vote_delegation::CardanoDRepType::deserialize(deserializer)?;
    Ok(drep_type as i32)
}

#[cfg(feature = "wasm")]
#[derive(serde::Deserialize)]
pub(crate) struct SerdeScriptConfig(pb::cardano_script_config::Config);

#[cfg(feature = "wasm")]
impl From<SerdeScriptConfig> for pb::CardanoScriptConfig {
    fn from(value: SerdeScriptConfig) -> Self {
        pb::CardanoScriptConfig {
            config: Some(value.0),
        }
    }
}

#[cfg(feature = "wasm")]
#[derive(serde::Deserialize)]
pub(crate) struct SerdeCert(pb::cardano_sign_transaction_request::certificate::Cert);

#[cfg(feature = "wasm")]
impl From<SerdeCert> for pb::cardano_sign_transaction_request::Certificate {
    fn from(value: SerdeCert) -> Self {
        pb::cardano_sign_transaction_request::Certificate {
            cert: Some(value.0),
        }
    }
}

/// Create a Shelley PaymentKeyHash/StakeKeyHash config.
/// <https://github.com/cardano-foundation/CIPs/blob/6c249ef48f8f5b32efc0ec768fadf4321f3173f2/CIP-0019/CIP-0019.md#shelley-addresses>
pub fn make_script_config_pkh_skh(
    keypath_payment: &Keypath,
    keypath_stake: &Keypath,
) -> pb::CardanoScriptConfig {
    pb::CardanoScriptConfig {
        config: Some(pb::cardano_script_config::Config::PkhSkh(
            pb::cardano_script_config::PkhSkh {
                keypath_payment: keypath_payment.to_vec(),
                keypath_stake: keypath_stake.to_vec(),
            },
        )),
    }
}

impl<R: Runtime> PairedBitBox<R> {
    async fn query_proto_cardano(
        &self,
        request: pb::cardano_request::Request,
    ) -> Result<pb::cardano_response::Response, Error> {
        self.validate_version(">=9.8.0")?; // Cardano since 9.8.0

        match self
            .query_proto(Request::Cardano(pb::CardanoRequest {
                request: Some(request),
            }))
            .await?
        {
            Response::Cardano(pb::CardanoResponse {
                response: Some(response),
            }) => Ok(response),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Does this device support Cardano functionality? Currently this means BitBox02 Multi.
    pub fn cardano_supported(&self) -> bool {
        matches!(self.product(), crate::Product::BitBox02Multi)
    }

    /// Query the device for xpubs. The result contains one xpub per requested keypath. Each xpub is
    /// 64 bytes: 32 byte chain code + 32 byte pubkey.
    pub async fn cardano_xpubs(&self, keypaths: &[Keypath]) -> Result<Vec<Vec<u8>>, Error> {
        match self
            .query_proto_cardano(pb::cardano_request::Request::Xpubs(
                pb::CardanoXpubsRequest {
                    keypaths: keypaths.iter().map(|kp| kp.into()).collect(),
                },
            ))
            .await?
        {
            pb::cardano_response::Response::Xpubs(pb::CardanoXpubsResponse { xpubs }) => Ok(xpubs),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Query the device for a Cardano address.
    pub async fn cardano_address(
        &self,
        network: pb::CardanoNetwork,
        script_config: &pb::CardanoScriptConfig,
        display: bool,
    ) -> Result<String, Error> {
        match self
            .query_proto_cardano(pb::cardano_request::Request::Address(
                pb::CardanoAddressRequest {
                    network: network.into(),
                    display,
                    script_config: Some(script_config.clone()),
                },
            ))
            .await?
        {
            pb::cardano_response::Response::Pub(pb::PubResponse { r#pub: address }) => Ok(address),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    /// Sign a Cardano transaction.
    pub async fn cardano_sign_transaction(
        &self,
        transaction: pb::CardanoSignTransactionRequest,
    ) -> Result<pb::CardanoSignTransactionResponse, Error> {
        match self
            .query_proto_cardano(pb::cardano_request::Request::SignTransaction(transaction))
            .await?
        {
            pb::cardano_response::Response::SignTransaction(response) => Ok(response),
            _ => Err(Error::UnexpectedResponse),
        }
    }
}
