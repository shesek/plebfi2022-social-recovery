use rand::Rng;
use std::fmt;
use std::convert::TryInto;

use bitcoin::hashes::hex::{FromHex, ToHex};
use bitcoin::util::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::{secp256k1, Network};
use minsc::bitcoin;
use serde::Deserialize;
use sharks::{Share, Sharks};

/// The recovery parameters. These are necessary for recovery and kept by both the user and his friends.
#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecoveryParams {
    total_shares: u8,
    needed_shares: u8,
    delay: u32,
    fee: u32,
}

/// The backup held by the user. This provides unconditional immediate control over the funds.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct UserBackup {
    params: RecoveryParams,
    user_seed: SeedSecret,
    recovery_xpub: ExtendedPubKey,
}

/// The complete data needed for recovery. This gets split into RecoveryShares
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RecoveryBackup {
    params: RecoveryParams,
    user_xpub: ExtendedPubKey,
    recovery_seed: SeedSecret,
}

/// A single share
pub struct RecoveryShare(sharks::Share);

pub type SeedSecret = [u8; 32];
pub type BackupBlob = Vec<u8>;

pub fn create_wallet(params: RecoveryParams, network: Network) -> (UserBackup, RecoveryBackup) {
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

    (user_backup, recovery_backup)
}

impl UserBackup {
    fn as_blob(&self) -> BackupBlob {
        bincode::serialize(self).unwrap()
    }
    fn as_hex(&self) -> String {
        self.as_blob().to_hex()
    }

    fn from_blob(blob: &BackupBlob) -> Result<Self, bincode::Error> {
        bincode::deserialize(&blob)
    }
    fn from_hex(s: &str) -> Result<Self, bincode::Error> {
        Self::from_blob(&Vec::from_hex(s).unwrap())
    }
}

impl RecoveryBackup {
    fn shares(&self) -> Vec<RecoveryShare> {
        let recovery_blob = bincode::serialize(&self).unwrap();
        let sharks = Sharks(self.params.needed_shares);
        let share_dealer = sharks.dealer(&recovery_blob);
        share_dealer
            .take(self.params.total_shares as usize)
            .map(RecoveryShare)
            .collect()
    }
}

impl RecoveryShare {
    fn as_blob(&self) -> BackupBlob {
        (&self.0).into()
    }
    fn as_hex(&self) -> String {
        self.as_blob().to_hex()
    }

    fn from_blob(blob: &BackupBlob) -> Result<Self, &'static str> {
        Ok(Self(blob[..].try_into()?))
    }
    fn from_hex(s: &str) -> Result<Self,&'static str> {
        Self::from_blob(&Vec::from_hex(s).unwrap())
    }
}

impl fmt::Debug for RecoveryShare {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_hex())
    }
}

#[test]
fn test_create_wallet() {
    use bitcoin::hashes::hex::ToHex;
    let params = RecoveryParams {
        total_shares: 7,
        needed_shares: 5,
        delay: 100,
        fee: 250,
    };
    let wallet = create_wallet(params, Network::Bitcoin);
    println!("user backup: {:?}", wallet.0);
    println!("user backup blob: {}", wallet.0.as_blob().to_hex());
    println!("recovery backup: {:?}", wallet.1);
    println!(
        "recovery shares: {:?}",
        wallet.1.shares().iter().map(|s| s.as_hex()).collect::<Vec<_>>()
    );

    println!(
        "user backup from hex: {:?}",
        UserBackup::from_hex(&wallet.0.as_hex())
    );
}
