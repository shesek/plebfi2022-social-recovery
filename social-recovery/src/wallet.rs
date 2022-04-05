use bitcoin::util::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::util::taproot::TaprootSpendInfo;
use bitcoin::{secp256k1, Amount, Network};
use minsc::bitcoin;

use minsc::runtime::{Evaluate, Execute};
use minsc::{parse_lib, Scope};

use crate::backup::{RecoveryParams, UserBackup};

lazy_static! {
    static ref MINSC_2SRECOVERY_LIB: minsc::ast::Library = parse_lib(
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
        }
    }

    pub fn tapinfo(&self, index: usize, amount: Amount) -> TaprootSpendInfo {
        let mut scope = Scope::root();

        MINSC_2SRECOVERY_LIB.exec(&mut scope).unwrap();

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

#[test]
fn test_wallet() {
    use crate::backup::create_wallet;
    let params = RecoveryParams {
        total_shares: 7,
        needed_shares: 5,
        delay: 100,
        fee: 250,
    };
    let (user_backup, recovery_backup) = create_wallet(params, Network::Signet);
    let wallet = UserWallet::from_backup(user_backup, Network::Signet);
    let tapinfo = wallet.tapinfo(0, "0.25 BTC".parse().unwrap());
    println!("address 0 tapinfo: {:?}", tapinfo);
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