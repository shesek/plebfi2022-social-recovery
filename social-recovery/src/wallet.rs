use rand::Rng;

use bitcoin::util::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::{secp256k1, Network};
use minsc::bitcoin;

struct WalletParams {}

#[derive(Debug, Copy, Clone)]
pub struct RecoveryParams {
    total_shares: usize,
    needed_shares: usize,
    delay: u32,
    fee: u32,
}

#[derive(Debug)]
pub struct UserBackup {
    params: RecoveryParams,
    user_xpriv: ExtendedPrivKey,
    recovery_xpub: ExtendedPubKey,
}

#[derive(Debug)]
pub struct RecoveryBackup {
    params: RecoveryParams,
    user_xpub: ExtendedPubKey,
    recovery_xpriv: ExtendedPrivKey,
}

pub fn create_wallet(params: RecoveryParams, network: Network) -> (UserBackup, RecoveryBackup) {
    let secp = secp256k1::Secp256k1::new();

    let user_seed = rand::thread_rng().gen::<[u8; 32]>();
    let recovery_seed = rand::thread_rng().gen::<[u8; 32]>();

    let user_xpriv = ExtendedPrivKey::new_master(network, &user_seed).unwrap();
    let recovery_xpriv = ExtendedPrivKey::new_master(network, &user_seed).unwrap();

    let user_backup = UserBackup {
        params,
        user_xpriv,
        recovery_xpub: ExtendedPubKey::from_priv(&secp, &recovery_xpriv),
    };

    let recovery_backup = RecoveryBackup {
        params,
        user_xpub: ExtendedPubKey::from_priv(&secp, &user_xpriv),
        recovery_xpriv,
    };

    (user_backup, recovery_backup)
}

#[test]
fn test_create_wallet() {
    let params = RecoveryParams {
        total_shares: 5,
        needed_shares: 5,
        delay: 100,
        fee: 250,
    };
    let wallet = create_wallet(params, Network::Bitcoin);
    println!("wallet: {:#?}", wallet);
}
