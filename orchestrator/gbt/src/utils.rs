use std::time::Duration;

use gravity_utils::types::{BatchRequestMode, RelayerConfig, ValsetRelayingMode};

pub const TIMEOUT: Duration = Duration::from_secs(60);

/// Explains the relaying config to users
pub fn print_relaying_explanation(input: &RelayerConfig, batch_requests: bool) {
    info!("Relaying from Cosmos => Ethereum is enabled, this will cost ETH");
    match input.valset_relaying_mode {
        ValsetRelayingMode::Altruistic => info!(
            "This relayer will relay validator set updates altruistically if required by the network"
        ),
        ValsetRelayingMode::EveryValset => warn!(
            "This relayer will relay every validator set update. "
        ),
    }
    match (input.batch_request_mode, batch_requests) {
        (_, false) => info!(
            "This relayer will not automatically request batches because the private key and fees are not provided",
        ),
        (BatchRequestMode::None, _) => info!(
            "This relayer will not automatically request batches, to enable this modify your configs `batch_request_mode`",
        ),
        (BatchRequestMode::ProfitableOnly, true) => info!(
            "This relayer will automatically spend tx fees to request the creation of batches that may be profitable",
        ),
        (BatchRequestMode::EveryBatch, true) => info!(
            "This relayer will automatically spend tx fees to request a batch when any tx are available",
        ),
    }
    match &input.batch_relaying_mode {
        gravity_utils::types::BatchRelayingMode::EveryBatch => {
            info!("This relayer will relay every batch. This will cost a lot of ETH!")
        },
        gravity_utils::types::BatchRelayingMode::ProfitableOnly { margin } => info!("This relayer will only relay batches if they have a profitable reward with at least {} margin", margin),
        gravity_utils::types::BatchRelayingMode::ProfitableWithWhitelist { margin, whitelist } =>
            info!("This relayer will relay profitable matches with {} margin, and the following tokens with the provided amounts {:?}", margin, whitelist)
    }
}
