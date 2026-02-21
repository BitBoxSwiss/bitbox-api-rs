// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "simulator")]
// Simulators only run on linux/amd64.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[cfg(not(feature = "tokio"))]
compile_error!("Enable the tokio feature to run simulator tests");

mod util;

use util::{test_initialized_simulators, test_simulators_after_pairing};

#[tokio::test]
async fn test_device_info() {
    test_simulators_after_pairing(async |paired_bitbox| {
        let device_info = paired_bitbox.device_info().await.unwrap();

        // Since v9.24.0, the simulator simulates a Nova device.
        if semver::VersionReq::parse(">=9.24.0")
            .unwrap()
            .matches(paired_bitbox.version())
        {
            assert_eq!(
                paired_bitbox.product(),
                bitbox_api::Product::BitBox02NovaMulti
            );
            assert_eq!(device_info.name, "BitBox HCXT")
        } else {
            assert_eq!(paired_bitbox.product(), bitbox_api::Product::BitBox02Multi);
            assert_eq!(device_info.name, "My BitBox")
        }
    })
    .await
}

#[tokio::test]
async fn test_root_fingerprint() {
    test_initialized_simulators(async |paired_bitbox| {
        assert_eq!(
            paired_bitbox.root_fingerprint().await.unwrap().as_str(),
            "4c00739d"
        );
    })
    .await
}

#[tokio::test]
async fn test_change_password() {
    test_initialized_simulators(async |bitbox| {
        if semver::VersionReq::parse(">=9.25.0")
            .unwrap()
            .matches(bitbox.version())
        {
            assert!(bitbox.change_password().await.is_ok());
        } else {
            assert!(matches!(
                bitbox.change_password().await,
                Err(bitbox_api::error::Error::Version(">=9.25.0"))
            ));
        }
    })
    .await
}
