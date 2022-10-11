//! Basic utility functions to stubbornly get data
use std::time::Duration;

use clarity::{Address as EthAddress, Uint256};
use deep_space::{address::Address as CosmosAddress, Coin, Contact};
use tokio::time::sleep;
use web30::client::Web3;

pub const RETRY_TIME: Duration = Duration::from_secs(5);

/// gets the current Ethereum finalized block number, no matter how long it takes
pub async fn get_finalized_block_number_with_retry(web3: &Web3) -> Uint256 {
    loop {
        match web3.eth_finalized_block_number().await {
            Ok(res) => return res,
            _ => sleep(RETRY_TIME).await,
        }
    }
}

// gets the latest block number
pub async fn get_latest_block_number_with_retry(web3: &Web3) -> Uint256 {
    loop {
        match web3.eth_block_number().await {
            Ok(res) => return res,
            _ => sleep(RETRY_TIME).await,
        }
    }
}

/// gets the current Ethereum block number, no matter how long it takes
pub async fn get_eth_balances_with_retry(address: EthAddress, web3: &Web3) -> Uint256 {
    loop {
        match web3.eth_get_balance(address).await {
            Ok(res) => return res,
            _ => sleep(RETRY_TIME).await,
        }
    }
}

/// gets Cosmos balances, no matter how long it takes
pub async fn get_balances_with_retry(address: CosmosAddress, contact: &Contact) -> Vec<Coin> {
    loop {
        match contact.get_balances(address).await {
            Ok(res) => return res,
            _ => sleep(RETRY_TIME).await,
        }
    }
}

/// gets the net version, no matter how long it takes
pub async fn get_net_version_with_retry(web3: &Web3) -> u64 {
    loop {
        match web3.net_version().await {
            Ok(res) => return res,
            _ => sleep(RETRY_TIME).await,
        }
    }
}
