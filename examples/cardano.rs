use bitbox_api::pb;

async fn demo<R: bitbox_api::runtime::Runtime>() {
    let noise_config = Box::new(bitbox_api::NoiseConfigNoCache {});
    let bitbox =
        bitbox_api::BitBox::<R>::from(bitbox_api::usb::get_any_bitbox02().unwrap(), noise_config)
            .await
            .unwrap();
    let pairing_bitbox = bitbox.unlock_and_pair().await.unwrap();
    if let Some(pairing_code) = pairing_bitbox.get_pairing_code().as_ref() {
        println!("Pairing code\n{}", pairing_code);
    }
    let paired_bitbox = pairing_bitbox.wait_confirm().await.unwrap();

    println!("Getting xpubs...");
    let xpubs = paired_bitbox
        .cardano_xpubs(&[
            "m/1852'/1815'/0'".try_into().unwrap(),
            "m/1852'/1815'/1'".try_into().unwrap(),
        ])
        .await
        .unwrap();
    println!("Xpubs: {:?}", xpubs);

    println!("Getting an address...");
    let address = paired_bitbox
        .cardano_address(
            pb::CardanoNetwork::CardanoMainnet,
            &bitbox_api::cardano::make_script_config_pkh_skh(
                &"m/1852'/1815'/0'/0/0".try_into().unwrap(),
                &"m/1852'/1815'/0'/2/0".try_into().unwrap(),
            ),
            true,
        )
        .await
        .unwrap();
    println!("Address: {}", address);

    println!("Signing a transaction with tokens...");
    let change_config = bitbox_api::cardano::make_script_config_pkh_skh(
        &"m/1852'/1815'/0'/1/0".try_into().unwrap(),
        &"m/1852'/1815'/0'/2/0".try_into().unwrap(),
    );
    let change_address = paired_bitbox
        .cardano_address(pb::CardanoNetwork::CardanoMainnet, &change_config, false)
        .await
        .unwrap();

    let keypath_input: bitbox_api::Keypath = "m/1852'/1815'/0'/0/0".try_into().unwrap();
    let transaction = pb::CardanoSignTransactionRequest {
        network: pb::CardanoNetwork::CardanoMainnet as i32,
        inputs: vec![
            pb::cardano_sign_transaction_request::Input {
                keypath: keypath_input.to_vec(),
                prev_out_hash: hex::decode("59864ee73ca5d91098a32b3ce9811bac1996dcbaefa6b6247dcaafb5779c2538").unwrap(),
                prev_out_index: 0,
            },
        ],
        outputs: vec![
            pb::cardano_sign_transaction_request::Output {
                encoded_address: "addr1q9qfllpxg2vu4lq6rnpel4pvpp5xnv3kvvgtxk6k6wp4ff89xrhu8jnu3p33vnctc9eklee5dtykzyag5penc6dcmakqsqqgpt".to_string(),
                value: 1000000,
                asset_groups: vec![
                    // Asset policy ids and asset names from:
                    // https://github.com/cardano-foundation/CIPs/blob/a2ef32d8a2b485fed7f6ffde2781dd58869ff511/CIP-0014/README.md#test-vectors
                    pb::cardano_sign_transaction_request::AssetGroup {
                        policy_id: hex::decode("1e349c9bdea19fd6c147626a5260bc44b71635f398b67c59881df209").unwrap(),
                        tokens: vec![
                            pb::cardano_sign_transaction_request::asset_group::Token {
                                asset_name: hex::decode("504154415445").unwrap(),
                                value: 1,
                            },
                            pb::cardano_sign_transaction_request::asset_group::Token {
                                asset_name: hex::decode("7eae28af2208be856f7a119668ae52a49b73725e326dc16579dcc373").unwrap(),
                                value: 3,
                            },
                        ],
                    },
                ],
                ..Default::default()
            },
            pb::cardano_sign_transaction_request::Output {
                encoded_address: change_address.clone(),
                value: 4829501,
                script_config: Some(change_config.clone()),
                ..Default::default()
        },
        ],
        fee: 170499,
        ttl: 41115811,
        certificates: vec![],
        withdrawals: vec![],
        validity_interval_start: 41110811,
        allow_zero_ttl: false,
    };

    let witness = paired_bitbox
        .cardano_sign_transaction(transaction)
        .await
        .unwrap();
    println!("Witness: {:?}", witness);

    println!("Delegating to a staking pool...");
    let transaction = pb::CardanoSignTransactionRequest {
        network: pb::CardanoNetwork::CardanoMainnet as i32,
        inputs: vec![pb::cardano_sign_transaction_request::Input {
            keypath: keypath_input.to_vec(),
            prev_out_hash: hex::decode(
                "59864ee73ca5d91098a32b3ce9811bac1996dcbaefa6b6247dcaafb5779c2538",
            )
            .unwrap(),
            prev_out_index: 0,
        }],
        outputs: vec![pb::cardano_sign_transaction_request::Output {
            encoded_address: change_address.clone(),
            value: 2741512,
            script_config: Some(change_config.clone()),
            ..Default::default()
        }],
        fee: 191681,
        ttl: 41539125,
        certificates: vec![
            pb::cardano_sign_transaction_request::Certificate {
                cert: Some(
                    pb::cardano_sign_transaction_request::certificate::Cert::StakeRegistration(
                        pb::Keypath {
                            keypath: vec![2147485500, 2147485463, 2147483648, 2, 0],
                        },
                    ),
                ),
            },
            pb::cardano_sign_transaction_request::Certificate {
                cert: Some(
                    pb::cardano_sign_transaction_request::certificate::Cert::StakeDelegation(
                        pb::cardano_sign_transaction_request::certificate::StakeDelegation {
                            keypath: vec![2147485500, 2147485463, 2147483648, 2, 0],
                            pool_keyhash: hex::decode(
                                "abababababababababababababababababababababababababababab",
                            )
                            .unwrap(),
                        },
                    ),
                ),
            },
        ],
        withdrawals: vec![],
        validity_interval_start: 41110811,
        allow_zero_ttl: false,
    };

    let witness = paired_bitbox
        .cardano_sign_transaction(transaction)
        .await
        .unwrap();
    println!("Witness: {:?}", witness);

    println!("Withdrawing staking rewards...");
    let transaction = pb::CardanoSignTransactionRequest {
        network: pb::CardanoNetwork::CardanoMainnet as i32,
        inputs: vec![pb::cardano_sign_transaction_request::Input {
            keypath: keypath_input.to_vec(),
            prev_out_hash: hex::decode(
                "59864ee73ca5d91098a32b3ce9811bac1996dcbaefa6b6247dcaafb5779c2538",
            )
            .unwrap(),
            prev_out_index: 0,
        }],
        outputs: vec![pb::cardano_sign_transaction_request::Output {
            encoded_address: change_address.clone(),
            value: 4817591,
            script_config: Some(change_config.clone()),
            ..Default::default()
        }],
        fee: 175157,
        ttl: 41788708,
        certificates: vec![],
        withdrawals: vec![pb::cardano_sign_transaction_request::Withdrawal {
            keypath: {
                let kp: bitbox_api::Keypath = "m/1852'/1815'/0'/2/0".try_into().unwrap();
                kp.to_vec()
            },
            value: 1234567,
        }],
        validity_interval_start: 0,
        allow_zero_ttl: false,
    };

    let witness = paired_bitbox
        .cardano_sign_transaction(transaction)
        .await
        .unwrap();
    println!("Witness: {:?}", witness);
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    demo::<bitbox_api::runtime::TokioRuntime>().await
}
