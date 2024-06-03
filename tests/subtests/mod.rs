pub type PairedBitBox = bitbox_api::PairedBitBox<bitbox_api::runtime::TokioRuntime>;

pub mod test_btc_psbt;

/// BIP32 xprv from BIP39 mnemonic used by the simulator:
/// boring mistake dish oyster truth pigeon viable emerge sort crash wire portion cannon couple enact box walk height pull today solid off enable tide
pub const SIMULATOR_BIP32_XPRV: &str = "xprv9s21ZrQH143K2qxpAMxVdyeza5dUBxY11XbJ7eKvRF51sQyhiFXgmn4P4ALi3Nf6bcG8cmPDvMMEFiAVjtXsqeZ47PJfBJif7uSYycMsx9c";

pub fn simulator_xprv() -> bitcoin::bip32::Xpriv {
    SIMULATOR_BIP32_XPRV.parse().unwrap()
}

pub fn simulator_xpub_at<C: bitcoin::secp256k1::Signing>(
    secp: &bitcoin::secp256k1::Secp256k1<C>,
    path: &bitcoin::bip32::DerivationPath,
) -> bitcoin::bip32::Xpub {
    bitcoin::bip32::Xpub::from_priv(secp, &simulator_xprv().derive_priv(secp, path).unwrap())
}
