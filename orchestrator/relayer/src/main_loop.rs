use std::time::Duration;

use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::{
    clarity::{address::Address as EthAddress, PrivateKey as EthPrivateKey},
    deep_space::{
        address::Address as CosmosAddress, Coin, Contact, PrivateKey as CosmosPrivateKey,
    },
    error::GravityError,
    types::RelayerConfig,
    web30::client::Web3,
};
use tokio::time::sleep;
use tonic::transport::Channel;

use crate::{
    batch_relaying::relay_batches, find_latest_valset::find_latest_valset,
    logic_call_relaying::relay_logic_calls, request_batches::request_batches,
    valset_relaying::relay_valsets,
};

pub const TIMEOUT: Duration = Duration::from_secs(10);

/// This function contains the orchestrator primary loop, it is broken out of the main loop so that
/// it can be called in the test runner for easier orchestration of multi-node tests
#[allow(clippy::too_many_arguments)]
pub async fn relayer_main_loop(
    ethereum_key: EthPrivateKey,
    cosmos_key: Option<CosmosPrivateKey>,
    cosmos_fee: Option<Coin>,
    web3: Web3,
    contact: Contact,
    grpc_client: GravityQueryClient<Channel>,
    gravity_contract_address: EthAddress,
    gravity_id: String,
    relayer_config: &RelayerConfig,
    reward_recipient: CosmosAddress,
) -> Result<(), GravityError> {
    let mut grpc_client = grpc_client;
    let loop_speed = Duration::from_secs(relayer_config.relayer_loop_speed);
    loop {
        let (async_result, _) = tokio::join!(
            async {
                let current_valset =
                    find_latest_valset(&mut grpc_client, gravity_contract_address, &web3).await;

                if current_valset.is_err() {
                    error!("Could not get current valset! {:?}", current_valset);
                    return Ok(());
                }

                let current_valset = current_valset.unwrap();

                relay_valsets(
                    &current_valset,
                    ethereum_key,
                    &web3,
                    &mut grpc_client,
                    gravity_contract_address,
                    gravity_id.clone(),
                    TIMEOUT,
                    relayer_config,
                    reward_recipient,
                )
                .await;

                relay_batches(
                    &current_valset,
                    ethereum_key,
                    &web3,
                    &mut grpc_client,
                    gravity_contract_address,
                    gravity_id.clone(),
                    TIMEOUT,
                    relayer_config,
                    reward_recipient,
                )
                .await;

                relay_logic_calls(
                    &current_valset,
                    ethereum_key,
                    &web3,
                    &mut grpc_client,
                    gravity_contract_address,
                    gravity_id.clone(),
                    TIMEOUT,
                    relayer_config,
                )
                .await;

                if let (Some(cosmos_key), Some(cosmos_fee)) = (cosmos_key, cosmos_fee.clone()) {
                    request_batches(
                        &contact,
                        &web3,
                        &mut grpc_client,
                        relayer_config.batch_request_mode,
                        cosmos_key,
                        cosmos_fee,
                    )
                    .await
                }

                Ok(())
            },
            // the sleep will be called in the parallel with the relay,
            // the "join!" will await for the longest operation time
            sleep(loop_speed),
        );

        if let Err(e) = async_result {
            return Err(e);
        }
    }
}
