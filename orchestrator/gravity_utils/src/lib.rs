//! This crate is for common functions and types for the Gravity rust code

#[macro_use]
extern crate log;

pub mod connection_prep;
pub mod error;
pub mod get_with_retry;
pub mod num_conversion;
pub mod prices;
pub mod types;

use std::env;

pub use clarity;
use clarity::{u256, Uint256};
pub use deep_space;
use get_with_retry::get_net_version_with_retry;
use lazy_static::lazy_static;
pub use u64_array_bigints;
pub use web30;
use web30::client::Web3;

// constants commonly modified across chains are here

// note: also modify the names in `module/config/config.go`
pub const DEFAULT_ADDRESS_PREFIX: &str = "onomy";
// note: also modify `GravityDenomPrefix` in `module/x/gravity/types/ethereum.go`
pub const GRAVITY_DENOM_PREFIX: &str = "eth";

// if the net version is this, the test values will be used
pub const TEST_ETH_CHAIN_ID: u64 = 15;

// see `orchestrator/src/ethereum_event_watcher.rs`

pub const BLOCK_DELAY: Uint256 = u256!(35);
pub const TEST_BLOCK_DELAY: Uint256 = u256!(0);

pub const USE_FINALIZATION: bool = false;

pub const DEFAULT_SEND_TO_COSMOS_MAX_GAS_LIMIT: Uint256 = u256!(100_000);
lazy_static! {
    // puts an absolute cap on the gas limit in send_to_cosmos
    pub static ref SEND_TO_COSMOS_MAX_GAS_LIMIT: Uint256 =
        env::var("SEND_TO_COSMOS_MAX_GAS_LIMIT").map(
            |s| Uint256::from_dec_or_hex_str(&s).expect(
                "SEND_TO_COSMOS_MAX_GAS_LIMIT is not a decimal \
                or hexadecimal string that fits in a Uint256"
        )
        ).unwrap_or_else(|_| DEFAULT_SEND_TO_COSMOS_MAX_GAS_LIMIT);
}

/// Only for tests, some chains are quiescent and need dummy transactions to keep block
/// production going and not softlock tests.
pub const TEST_RUN_BLOCK_STIMULATOR: bool = false;
pub const TEST_DEFAULT_MINER_KEY: &str =
    "0xb1bab011e03a9862664706fc3bbaa1b16651528e5f0e7fbfcbfdd8be302a13e7";
pub const TEST_DEFAULT_ETH_NODE_ENDPOINT: &str = "http://localhost:8545";
pub const TEST_FEE_AMOUNT: Uint256 = u256!(1_000_000_000);
pub const TEST_GAS_LIMIT: Uint256 = u256!(200_000);
pub const TEST_INVALID_EVENTS_GAS_LIMIT: Uint256 = u256!(7_000_000);
/// When debugging `BATCH_STRESS` or `REMOTE_STRESS` it may be useful to reduce this,
/// note this has a minimum of 4 users because of assumptions the tests make
pub const TESTS_BATCH_NUM_USERS: usize = 100;
/// This causes failures in INVALID_EVENTS if too large
pub const TEST_ERC20_MAX_SIZE: usize = 3_000;

/// For chains with probabilistic finality (`USE_FINALIZATION == false`),
/// this will delay `check_for_events` from considering a block finalized
/// until a conservative number of blocks have passed.
pub async fn get_block_delay(web3: &Web3) -> Uint256 {
    let net_version = get_net_version_with_retry(web3).await;

    match net_version {
        TEST_ETH_CHAIN_ID => TEST_BLOCK_DELAY,
        _ => BLOCK_DELAY,
    }
}
