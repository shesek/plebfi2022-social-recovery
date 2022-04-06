use wasm_bindgen::prelude::*;

use minsc::bitcoin::Network;

use crate::backup::{create_wallet, RecoveryParams};


#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(js_name = create_wallet)]
pub fn js_create_wallet(
    total_shares: u32,
    needed_shares: u32,
    delay: &str,
) -> std::result::Result<JsValue, JsValue> {
    let params = RecoveryParams {
        total_shares: total_shares as u8,
        needed_shares: needed_shares as u8,
        delay: minsc::run(delay).unwrap().into_u32().unwrap(),
        fee: 250,
    };
    let (user_backup, recovery_backup) = create_wallet(params, Network::Signet);

    Ok(JsValue::from_serde(&json!({

        "params": params,
        "user_backup_hex": user_backup.as_hex(),
        "shares": recovery_backup.split_shares().into_iter().map(|s| s.as_hex()).collect::<Vec<_>>()
    })).unwrap())
}
