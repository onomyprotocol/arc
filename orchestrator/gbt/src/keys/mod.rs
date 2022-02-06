pub mod register_orchestrator_address;

use crate::{
    args::{SetEthereumKeyOpts, SetOrchestratorKeyOpts},
    config::{config_exists, load_keys, save_keys},
};
use deep_space::PrivateKey;
use gravity_utils::error::GravityError;
use std::path::Path;

pub fn show_keys(home_dir: &Path, prefix: &str) -> Result<(), GravityError> {
    if !config_exists(home_dir) {
        return Err(GravityError::UnrecoverableError(
            "Please run `gbt init` before running this command!".into(),
        ));
    }
    let keys = load_keys(home_dir)?;

    match keys.orchestrator_phrase {
        Some(v) => {
            let key = PrivateKey::from_phrase(&v, "")
                .expect("Failed to decode key in keyfile. Did you edit it manually?");
            let address = key.to_address(prefix).unwrap();
            info!("Your Orchestrator key, {}", address);
        }
        None => info!("You do not have an Orchestrator key set"),
    }
    match keys.ethereum_key {
        Some(v) => {
            let address = v.to_address();
            info!("Your Ethereum key, {}", address);
        }
        None => info!("You do not have an Ethereum key set"),
    }

    Ok(())
}

pub fn set_eth_key(home_dir: &Path, opts: SetEthereumKeyOpts) -> Result<(), GravityError> {
    if !config_exists(home_dir) {
        return Err(GravityError::UnrecoverableError(
            "Please run `gbt init` before running this command!".into(),
        ));
    }
    let mut keys = load_keys(home_dir)?;
    keys.ethereum_key = Some(opts.key);
    save_keys(home_dir, keys);
    info!("Successfully updated Ethereum Key");

    Ok(())
}

pub fn set_orchestrator_key(
    home_dir: &Path,
    opts: SetOrchestratorKeyOpts,
) -> Result<(), GravityError> {
    if !config_exists(home_dir) {
        return Err(GravityError::UnrecoverableError(
            "Please run `gbt init` before running this command!".into(),
        ));
    }
    let res = PrivateKey::from_phrase(&opts.phrase, "");
    if let Err(e) = res {
        return Err(GravityError::UnrecoverableError(format!(
            "Invalid Cosmos mnemonic phrase {} {:?}",
            opts.phrase, e
        )));
    }
    let mut keys = load_keys(home_dir)?;
    keys.orchestrator_phrase = Some(opts.phrase);
    save_keys(home_dir, keys);
    info!("Successfully updated Orchestrator Key");

    Ok(())
}
