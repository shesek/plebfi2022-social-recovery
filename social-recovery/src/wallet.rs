use rand::Rng;

use bitcoin::util::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::{secp256k1, Network};
use minsc::bitcoin;
use sharks::{Share, Sharks};

#[derive(Debug, Copy, Clone, serde::Serialize)]
pub struct RecoveryParams {
    total_shares: u8,
    needed_shares: u8,
    delay: u32,
    fee: u32,
}

#[derive(Debug, serde::Serialize)]
pub struct UserBackup {
    params: RecoveryParams,
    user_xpriv: ExtendedPrivKey,
    recovery_xpub: ExtendedPubKey,
}

#[derive(Debug, serde::Serialize)]
pub struct RecoveryBackup {
    params: RecoveryParams,
    user_xpub: ExtendedPubKey,
    recovery_xpriv: ExtendedPrivKey,
}

pub type BackupBlob = Vec<u8>;
pub type RecoveryShares = Vec<BackupBlob>;

pub fn create_wallet(
    params: RecoveryParams,
    network: Network,
) -> (UserBackup, RecoveryBackup, BackupBlob, RecoveryShares) {
    let secp = secp256k1::Secp256k1::new();

    let user_seed = rand::thread_rng().gen::<[u8; 32]>();
    let recovery_seed = rand::thread_rng().gen::<[u8; 32]>();

    let user_xpriv = ExtendedPrivKey::new_master(network, &user_seed).unwrap();
    let recovery_xpriv = ExtendedPrivKey::new_master(network, &recovery_seed).unwrap();

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

    let user_blob = bincode::serialize(&user_backup).unwrap();
    let recovery_blob = bincode::serialize(&recovery_backup).unwrap();

    let sharks = Sharks(params.needed_shares);
    let share_dealer = sharks.dealer(&recovery_blob);
    let shares: Vec<Share> = share_dealer.take(params.total_shares as usize).collect();

    let recovery_shares_blobs: Vec<Vec<u8>> = shares.iter().map(Vec::from).collect();

    (
        user_backup,
        recovery_backup,
        user_blob,
        recovery_shares_blobs,
    )
}

#[test]
fn test_create_wallet() {
    use bitcoin::hashes::hex::ToHex;
    let params = RecoveryParams {
        total_shares: 5,
        needed_shares: 5,
        delay: 100,
        fee: 250,
    };
    let wallet = create_wallet(params, Network::Bitcoin);
    println!("user backup: {:#?}", wallet.0);
    println!("recovery backup: {:#?}", wallet.1);

    println!("user backup blob: {}", wallet.2.to_hex());
    println!(
        "recovery shares: {:?}",
        wallet.3.iter().map(|s| s.to_hex()).collect::<Vec<_>>()
    );
}
