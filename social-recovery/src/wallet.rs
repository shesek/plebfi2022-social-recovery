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
    user_seed: SeedSecret,
    recovery_xpub: ExtendedPubKey,
}

#[derive(Debug, serde::Serialize)]
pub struct RecoveryBackup {
    params: RecoveryParams,
    user_xpub: ExtendedPubKey,
    recovery_seed: SeedSecret,
}

pub type SeedSecret = [u8; 32];
pub type BackupBlob = Vec<u8>;
pub type RecoveryShares = Vec<BackupBlob>;

pub fn create_wallet(
    params: RecoveryParams,
    network: Network,
) -> (UserBackup, RecoveryBackup, RecoveryShares) {
    let secp = secp256k1::Secp256k1::new();

    let user_seed = rand::thread_rng().gen::<[u8; 32]>();
    let recovery_seed = rand::thread_rng().gen::<[u8; 32]>();

    let user_xpriv = ExtendedPrivKey::new_master(network, &user_seed).unwrap();
    let recovery_xpriv = ExtendedPrivKey::new_master(network, &recovery_seed).unwrap();

    let user_backup = UserBackup {
        params,
        user_seed,
        recovery_xpub: ExtendedPubKey::from_priv(&secp, &recovery_xpriv),
    };

    let recovery_backup = RecoveryBackup {
        params,
        user_xpub: ExtendedPubKey::from_priv(&secp, &user_xpriv),
        recovery_seed,
    };

    let recovery_blob = bincode::serialize(&recovery_backup).unwrap();

    let sharks = Sharks(params.needed_shares);
    let share_dealer = sharks.dealer(&recovery_blob);
    let shares: Vec<Share> = share_dealer.take(params.total_shares as usize).collect();

    let recovery_shares_blobs: Vec<Vec<u8>> = shares.iter().map(Vec::from).collect();

    (user_backup, recovery_backup, recovery_shares_blobs)
}

impl UserBackup {
    fn as_blob(&self) -> BackupBlob {
        bincode::serialize(self).unwrap()
    }

    /*
    fn as_bip39_mnemonic(&self) -> Mnemonic {
        let mut blob = self.as_blob();
        // Has to be a multiply of 32 bits to be converted into a mnemonic. Pad it with 3 extra 0x00 to make it so.
        assert_eq!(blob.len(), 161);
        blob.insert(0, 0);
        blob.insert(0, 0);
        blob.insert(0, 0);
        Mnemonic::from_entropy(&blob).unwrap()
    }

    fn from_bip39_mnemonic(s: &str) -> Result<Self, bip39::Error> {
        let mnemonic = Mnemonic::parse(s)?;
        let mut bytes = mnemonic.to_entropy();
        assert_eq!(bytes.len(), 164);
        // drop the extra 0x00 padding
        for _ in 0..3 {
            assert_eq!(bytes[0], 0);
            bytes.remove(0);
        }
        Ok(bincode::deserialize(&bytes).unwrap())
    }
    */
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
    println!("user backup blob: {}", wallet.0.as_blob().to_hex());
    //println!("user backup mnemonic: {}", wallet.0.as_bip39_mnemonic());
    println!("recovery backup: {:#?}", wallet.1);

    println!(
        "recovery shares: {:?}",
        wallet.2.iter().map(|s| s.to_hex()).collect::<Vec<_>>()
    );
}
