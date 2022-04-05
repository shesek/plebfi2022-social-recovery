use bitcoin::util::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::util::taproot::TaprootSpendInfo;
use bitcoin::{secp256k1, Amount};
use minsc::bitcoin;

use minsc::{Evaluate, Scope};

use crate::backup::{RecoveryParams, UserBackup};

pub struct UserWallet {
    params: RecoveryParams,
    user_xpriv: ExtendedPrivKey,
    user_xpub: ExtendedPubKey,
    recovery_xpub: ExtendedPubKey,
}

impl UserWallet {
    pub fn from_backup(backup: UserBackup, network: bitcoin::Network) -> Self {
        let secp = secp256k1::Secp256k1::new();

        let user_xpriv = ExtendedPrivKey::new_master(network, &backup.user_seed).unwrap();
        let user_xpub = ExtendedPubKey::from_priv(&secp, &user_xpriv);

        Self {
            params: backup.params,
            user_xpriv,
            user_xpub,
            recovery_xpub: backup.recovery_xpub,
        }
    }

    pub fn tapinfo(&self, index: usize, amount: Amount) -> TaprootSpendInfo {
        let mut scope = Scope::root();

        //scope.include_lib("./two-step-recovery.minsc");

        scope.set("$user_xpub", self.user_xpub).unwrap();
        scope.set("$recovery_xpub", self.recovery_xpub).unwrap();
        scope.set("$delay", self.params.delay as i64).unwrap();
        scope.set("$fee", self.params.fee as i64).unwrap();

        scope.set("$index", index as i64).unwrap();
        scope.set("$amount", amount.as_sat() as i64).unwrap();

        minsc::parse(
            "twoStepRecovery($user_xpub/$index, $recovery_xpub/$index, $delay, $amount, $fee)",
        )
        .unwrap()
        .eval(&scope)
        .unwrap()
        .into_tapinfo()
        .unwrap()
    }

    // TODO pub fn output_keypair(&self, index, amount) -> (output_xonlypubkey, output_tweaked_privkey)
}
