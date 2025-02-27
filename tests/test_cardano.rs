#![cfg(feature = "simulator")]
// Simulators only run on linux/amd64.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[cfg(not(feature = "tokio"))]
compile_error!("Enable the tokio feature to run simulator tests");

mod util;

use util::test_initialized_simulators;

use bitbox_api::pb;

#[tokio::test]
async fn test_cardano_xpubs() {
    test_initialized_simulators(async |bitbox| {
        assert!(bitbox.cardano_supported());

        let xpubs = bitbox
            .cardano_xpubs(&[
                "m/1852'/1815'/0'".try_into().unwrap(),
                "m/1852'/1815'/1'".try_into().unwrap(),
            ])
            .await
            .unwrap();
        assert_eq!(xpubs.len(), 2);
        assert_eq!(
            hex::encode(&xpubs[0]),
            "9fc9550e8379cb97c2d2557d89574207c6cf4d4ff62b37e377f2b3b3c284935b677f0fe5a4a6928c7b982c0c149f140c26c0930b73c2fe16feddfa21625e0316",
        );
        assert_eq!(
            hex::encode(&xpubs[1]),
            "7ffd0bd7d54f1648ac59a357d3eb27b878c2f7c09739d3b7c7e6662d496dea16f10ef525258833d37db047cd530bf373ebcb283495aa4c768424a2af37cee661",
        );
    }).await
}

#[tokio::test]
async fn test_cardano_address() {
    test_initialized_simulators(async |bitbox| {
        let address = bitbox
            .cardano_address(
                pb::CardanoNetwork::CardanoMainnet,
                &bitbox_api::cardano::make_script_config_pkh_skh(
                    &"m/1852'/1815'/0'/0/0".try_into().unwrap(),
                    &"m/1852'/1815'/0'/2/0".try_into().unwrap(),
                ),
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            address,
            "addr1qxz808eh7aw8cwjhlxlzu4p3ct299qrzjlnp7pwvh7nc9hg0342h3nhc8vnf6c93wnxgqv3xztkfq7cnjegcqz30vg7s3sx0l4",
        );
    }).await
}

#[tokio::test]
async fn test_cardano_transactions() {
    test_initialized_simulators(async |bitbox| {
        let change_config = bitbox_api::cardano::make_script_config_pkh_skh(
            &"m/1852'/1815'/0'/1/0".try_into().unwrap(),
            &"m/1852'/1815'/0'/2/0".try_into().unwrap(),
        );
        let change_address = bitbox
            .cardano_address(pb::CardanoNetwork::CardanoMainnet, &change_config, false)
            .await
            .unwrap();
        let keypath_input: bitbox_api::Keypath = "m/1852'/1815'/0'/0/0".try_into().unwrap();

        // Sign a transaction with tokens
        {
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
                tag_cbor_sets: false,
            };

            let witness = bitbox.cardano_sign_transaction(transaction).await.unwrap();
            assert_eq!(witness.shelley_witnesses.len(), 1);
            assert_eq!(
                hex::encode(&witness.shelley_witnesses[0].public_key),
                "6b5d4134cfc66281827d51cb0196f1a951ce168c19ba1314233f43d39d91e2bc",
            );
        }
        // Delegating to a staking pool...
        {
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
                tag_cbor_sets: false,
            };

            let witness = bitbox.cardano_sign_transaction(transaction).await.unwrap();
            assert_eq!(witness.shelley_witnesses.len(), 2);
            assert_eq!(
                hex::encode(&witness.shelley_witnesses[0].public_key),
                "6b5d4134cfc66281827d51cb0196f1a951ce168c19ba1314233f43d39d91e2bc",
            );
            assert_eq!(
                hex::encode(&witness.shelley_witnesses[1].public_key),
                "ed0d6426efcae3b02b963db0997845ba43ed53c131aa2f0faa01976ddcdb3751",
            );
        }
        // Delegating vote to a drep
        {
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
                            pb::cardano_sign_transaction_request::certificate::Cert::VoteDelegation(
                                pb::cardano_sign_transaction_request::certificate::VoteDelegation {
                                    keypath: vec![2147485500, 2147485463, 2147483648, 2, 0],
                                    r#type: pb::cardano_sign_transaction_request::certificate::vote_delegation::CardanoDRepType::AlwaysAbstain.into(),
                                    drep_credhash: None,
                                },
                            ),
                        ),
                    },
                ],
                withdrawals: vec![],
                validity_interval_start: 41110811,
                allow_zero_ttl: false,
                tag_cbor_sets: false,
            };

            if semver::VersionReq::parse(">=9.21.0")
                .unwrap()
                .matches(bitbox.version())
            {
                let witness = bitbox.cardano_sign_transaction(transaction).await.unwrap();
                assert_eq!(witness.shelley_witnesses.len(), 2);
                assert_eq!(
                    hex::encode(&witness.shelley_witnesses[0].public_key),
                    "6b5d4134cfc66281827d51cb0196f1a951ce168c19ba1314233f43d39d91e2bc",
                );
                assert_eq!(
                    hex::encode(&witness.shelley_witnesses[1].public_key),
                    "ed0d6426efcae3b02b963db0997845ba43ed53c131aa2f0faa01976ddcdb3751",
                );
            }
        }
        // Delegating vote to a drep with a keyhash
        {
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
                            pb::cardano_sign_transaction_request::certificate::Cert::VoteDelegation(
                                pb::cardano_sign_transaction_request::certificate::VoteDelegation {
                                    keypath: vec![2147485500, 2147485463, 2147483648, 2, 0],
                                    r#type: pb::cardano_sign_transaction_request::certificate::vote_delegation::CardanoDRepType::KeyHash.into(),
                                    drep_credhash: Some(hex::decode(
                                        "abababababababababababababababababababababababababababab",
                                    )
                                                        .unwrap()),
                                },
                            ),
                        ),
                    },
                ],
                withdrawals: vec![],
                validity_interval_start: 41110811,
                allow_zero_ttl: false,
                tag_cbor_sets: false,
            };

            if semver::VersionReq::parse(">=9.21.0")
                .unwrap()
                .matches(bitbox.version())
            {
                let witness = bitbox.cardano_sign_transaction(transaction).await.unwrap();
                assert_eq!(witness.shelley_witnesses.len(), 2);
                assert_eq!(
                    hex::encode(&witness.shelley_witnesses[0].public_key),
                    "6b5d4134cfc66281827d51cb0196f1a951ce168c19ba1314233f43d39d91e2bc",
                );
                assert_eq!(
                    hex::encode(&witness.shelley_witnesses[1].public_key),
                    "ed0d6426efcae3b02b963db0997845ba43ed53c131aa2f0faa01976ddcdb3751",
                );
            }
        }
        // Withdrawing staking rewards...
        {
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
                tag_cbor_sets: false,
            };

            let witness = bitbox.cardano_sign_transaction(transaction).await.unwrap();
            assert_eq!(witness.shelley_witnesses.len(), 2);
            assert_eq!(
                hex::encode(&witness.shelley_witnesses[0].public_key),
                "6b5d4134cfc66281827d51cb0196f1a951ce168c19ba1314233f43d39d91e2bc",
            );
            assert_eq!(
                hex::encode(&witness.shelley_witnesses[1].public_key),
                "ed0d6426efcae3b02b963db0997845ba43ed53c131aa2f0faa01976ddcdb3751",
            );
        }
        // Using 258-tagged cbor sets
        {
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
                tag_cbor_sets: true,
            };
            if semver::VersionReq::parse(">=9.22.0")
                .unwrap()
                .matches(bitbox.version())
            {
                let witness = bitbox.cardano_sign_transaction(transaction).await.unwrap();
                assert_eq!(witness.shelley_witnesses.len(), 1);
                assert_eq!(
                    hex::encode(&witness.shelley_witnesses[0].public_key),
                    "6b5d4134cfc66281827d51cb0196f1a951ce168c19ba1314233f43d39d91e2bc",
                );
            } else {
                assert!(matches!(
                    bitbox.cardano_sign_transaction(transaction).await,
                    Err(bitbox_api::error::Error::Version(">=9.22.0"))
                ));
            }
        }
    }).await
}
