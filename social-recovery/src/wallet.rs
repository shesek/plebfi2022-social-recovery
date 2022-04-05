use bitcoin::util::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::util::schnorr::{TapTweak, TweakedKeyPair, TweakedPublicKey};
use bitcoin::util::taproot::TaprootSpendInfo;
use bitcoin::{secp256k1, Address, Amount, Network};
use minsc::bitcoin;

use minsc::runtime::{Evaluate, Execute};

use crate::backup::{RecoveryParams, UserBackup};

lazy_static! {
    static ref MINSC_2SRECOVERY_LIB: minsc::ast::Library = minsc::parse_lib(
        r#"
        fn twoStepRecovery($user_pk, $recovery_pk, $delay, $amount, $fee) =
          $user_pk+(pk($recovery_pk) && txtmpl([
            txOut($user_pk+(pk($recovery_pk) && older($delay)), $amount - $fee - DUST_AMOUNT),
            txOut($user_pk+pk($recovery_pk), DUST_AMOUNT) // anchor output for fee bumping
          ]));
        "#
    )
    .unwrap();
    pub static ref EC: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
}

pub struct UserWallet {
    params: RecoveryParams,
    user_xpriv: ExtendedPrivKey,
    user_xpub: ExtendedPubKey,
    recovery_xpub: ExtendedPubKey,
    network: Network,
}

impl UserWallet {
    pub fn from_backup(backup: UserBackup, network: Network) -> Self {
        let user_xpriv = ExtendedPrivKey::new_master(network, &backup.user_seed).unwrap();
        let user_xpub = ExtendedPubKey::from_priv(&EC, &user_xpriv);

        Self {
            params: backup.params,
            user_xpriv,
            user_xpub,
            recovery_xpub: backup.recovery_xpub,
            network,
        }
    }

    pub fn address_pks(&self, index: u32) -> (ExtendedPubKey, ExtendedPubKey) {
        let user_pk = self.user_xpub.derive_pub(&EC, &[index.into()]).unwrap();
        let recovery_pk = self.recovery_xpub.derive_pub(&EC, &[index.into()]).unwrap();
        (user_pk, recovery_pk)
    }

    pub fn eval_minsc(&self, index: u32, amount: Amount, code: &str) -> minsc::Value {
        let mut scope = minsc::Scope::root();
        MINSC_2SRECOVERY_LIB.exec(&mut scope).unwrap();

        let (user_pk, recovery_pk) = self.address_pks(index);
        scope.set("$user_pk", user_pk).unwrap();
        scope.set("$recovery_pk", recovery_pk).unwrap();
        scope.set("$delay", self.params.delay as i64).unwrap();
        scope.set("$fee", self.params.fee as i64).unwrap();

        scope.set("$index", index as i64).unwrap();
        scope.set("$amount", amount.as_sat() as i64).unwrap();

        minsc::parse(code).unwrap().eval(&scope).unwrap()
    }

    pub fn tapinfo(&self, index: u32, amount: Amount) -> TaprootSpendInfo {
        self.eval_minsc(
            index,
            amount,
            "twoStepRecovery($user_pk, $recovery_pk, $delay, $amount, $fee)",
        )
        .into_tapinfo()
        .unwrap()
    }

    pub fn tweaked_output_keypair(&self, index: u32, amount: Amount) -> (TweakedKeyPair) {
        let user_priv = self.user_xpriv.derive_priv(&EC, &[index.into()]).unwrap();
        let merkle_root = self.tapinfo(index, amount).merkle_root();

        user_priv.to_keypair(&EC).tap_tweak(&EC, merkle_root)
    }

    pub fn tweaked_output_pubkey(
        &self,
        index: u32,
        amount: Amount,
    ) -> (TweakedPublicKey, secp256k1::Parity) {
        let user_pub = self.user_xpub.derive_pub(&EC, &[index.into()]).unwrap();
        let merkle_root = self.tapinfo(index, amount).merkle_root();

        user_pub.to_x_only_pub().tap_tweak(&EC, merkle_root)
    }

    pub fn address(&self, index: u32, amount: Amount) -> Address {
        let (tweaked_pubkey, _) = self.tweaked_output_pubkey(index, amount);
        Address::p2tr_tweaked(tweaked_pubkey, self.network)
    }

    pub fn export_tweaked(
        &self,
        start_index: u32,
        end_index: u32,
        amounts: &[Amount],
    ) -> Vec<TweakedKeyPair> {
        let mut keypairs =
            Vec::with_capacity((end_index - start_index + 1) as usize * amounts.len());
        for index in start_index..=end_index {
            for amount in amounts {
                keypairs.push(self.tweaked_output_keypair(index, *amount));
            }
        }
        keypairs
    }
}

#[test]
fn test_wallet() {
    use crate::backup::create_wallet;
    let params = RecoveryParams {
        total_shares: 7,
        needed_shares: 5,
        delay: 100,
        fee: 250,
    };
    let (user_backup, _) = create_wallet(params, Network::Signet);
    let wallet = UserWallet::from_backup(user_backup, Network::Signet);
    let tapinfo = wallet.tapinfo(0, "0.25 BTC".parse().unwrap());
    println!("address 0 tapinfo: {:?}", tapinfo);
    let keypair = wallet.tweaked_output_keypair(0, "0.25 BTC".parse().unwrap());
    println!("address 0 keypair: {:?}", keypair);
    let pubkey = wallet.tweaked_output_pubkey(0, "0.25 BTC".parse().unwrap());
    println!("address 0 pubkey: {:?}", pubkey);
    println!(
        "address 0: {:?}",
        wallet.address(0, "0.25 BTC".parse().unwrap())
    );
    println!(
        "export 0-3 with 2 amounts: {} total keypairs",
        wallet
            .export_tweaked(
                0,
                3,
                &["0.25 BTC".parse().unwrap(), "1 BTC".parse().unwrap()]
            )
            .len()
    );
}

// slides
// - problem lost keys, solution recovery, problem collusion, solution delay via CTV
// - what's possible on bitcoin today with CSV (minsc code)
// - two-step recovery with CTV (minsc code)
// - assumptions, why SSSS
// - simplified two-step recovery with taproot (+taproot advantages, ssss advantage over taproot musig leaves)
// - demo
// - limitations / future work
//   - no ctv-capable wallets
//   - static backups are tricky, some solutions:
//     - multiple fixed denominations
//     - scan all outputs
//     - mark with OP_RETURN
//     - elements-style introspection
//     - CTV relative-to-total amounts (also helps prevent stuck funds due to amount mismiatch)
//     - accept backups not being static