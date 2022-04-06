use serde_json::Value;

use bitcoin::util::bip32::{ExtendedPrivKey, ExtendedPubKey};
use bitcoin::util::schnorr::{TapTweak, TweakedKeyPair, TweakedPublicKey};
use bitcoin::util::taproot::TaprootSpendInfo;
use bitcoin::{secp256k1, Address, Amount, Network};
//use bitcoincore_rpc::RpcApi;
use minsc::{bitcoin, miniscript};

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
        let tapinfo = self.eval_minsc(
            index,
            amount,
            "twoStepRecovery($user_pk, $recovery_pk, $delay, $amount, $fee)",
        )
        .into_tapinfo()
        .unwrap();
        println!("Created taproot tree for index {} amount {}:", index, amount);
        println!("{:?}\n\n", tapinfo);

        use bitcoin::util::address::WitnessVersion;
        let spk =  bitcoin::Script::new_witness_program(WitnessVersion::V1, &tapinfo.output_key().serialize());
        println!("Address {}: {}\n\n", index, Address::from_script(&spk, self.network).unwrap());
        tapinfo
    }

    pub fn tweaked_output_keypair(&self, index: u32, amount: Amount) -> TweakedKeyPair {
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

    /* 
    pub fn import_tweaked_to_core(
        &self,
        client: &bitcoincore_rpc::Client,
        start_index: u32,
        end_index: u32,
        amounts: &[Amount],
    ) -> Result<(), ()> {
        let keypairs = self.export_tweaked(start_index, end_index, amounts);
        for keypair in keypairs {
            let keypairi = keypair.into_inner();
            let seckey = secp256k1::SecretKey::from_keypair(&keypairi);
            let corerpc_seckey =
                bitcoincore_rpc::bitcoin::secp256k1::SecretKey::from_slice(seckey.as_ref())
                    .unwrap();
            let privkey = bitcoincore_rpc::bitcoin::PrivateKey {
                compressed: true,
                network: bitcoincore_rpc::bitcoin::Network::Signet, // FIXME self.network,
                key: corerpc_seckey,
            };
            println!("privkey: {}", privkey.to_wif());
            let desc = format!("rawtr({})", privkey.to_wif());
            let checksum = crate::desc_checksum(&desc);
            println!("descriptor: {}#{}", desc, checksum);
            client
                .call::<Value>(
                    "importdescriptors",
                    &[json!([{
                        "desc": format!("{}#{}", desc, checksum),
                        "timestamp": "now",
                    }])],
                )
                .unwrap();
        }
        client.rescan_blockchain(None, None).unwrap();
        Ok(())
    }
    */

    //pub fn staging_descriptor_xpriv(&self, index: u32, amount: Amount) -> String {
    //    self.eval_minsc(index, amount, "")
    //}
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

    let amounts = vec!["0.25 BTC".parse().unwrap(), "1 BTC".parse().unwrap()];

    println!(
        "export 0-3 with 2 amounts: {} total keypairs",
        wallet.export_tweaked(0, 4, &amounts).len()
    );

    /* 
    let client = bitcoincore_rpc::Client::new(
        "http://127.0.0.1:38332/wallet/ctvex",
        bitcoincore_rpc::Auth::UserPass("satoshi".into(), "1234".into()),
    )
    .unwrap();
    wallet
        .import_tweaked_to_core(&client, 0, 3, &amounts)
        .unwrap();
        */
}