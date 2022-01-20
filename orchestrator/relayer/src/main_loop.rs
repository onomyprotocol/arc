use crate::{
    batch_relaying::relay_batches, find_latest_valset::find_latest_valset,
    logic_call_relaying::relay_logic_calls, valset_relaying::relay_valsets,
};
use clarity::address::Address as EthAddress;
use clarity::PrivateKey as EthPrivateKey;
use ethereum_gravity::utils::get_gravity_id_with_retry;
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::{error::GravityError, types::RelayerConfig};
use std::time::Duration;
use tokio::time::sleep;
use tonic::transport::Channel;
use web30::client::Web3;

pub const LOOP_SPEED: Duration = Duration::from_secs(17);

/// This function contains the orchestrator primary loop, it is broken out of the main loop so that
/// it can be called in the test runner for easier orchestration of multi-node tests
pub async fn relayer_main_loop(
    ethereum_key: EthPrivateKey,
    web3: Web3,
    grpc_client: GravityQueryClient<Channel>,
    gravity_contract_address: EthAddress,
    relayer_config: &RelayerConfig,
    wait_time: Option<Duration>,
) -> Result<(), GravityError> {
    let mut grpc_client = grpc_client;
    let our_ethereum_address = ethereum_key.to_address();

    let gravity_id = get_gravity_id_with_retry(
        gravity_contract_address,
        our_ethereum_address,
        &web3,
        wait_time,
    )
    .await;

    // timeout expired - ethreum node not reachable
    if gravity_id.is_err() {
        return Err(GravityError::UnrecoverableError(
            "Failed to get GravityID, check your ethereum node".into(),
        ));
    }

    let gravity_id = gravity_id.unwrap();

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
                    LOOP_SPEED,
                    relayer_config,
                )
                .await;

                relay_batches(
                    &current_valset,
                    ethereum_key,
                    &web3,
                    &mut grpc_client,
                    gravity_contract_address,
                    gravity_id.clone(),
                    LOOP_SPEED,
                    relayer_config,
                )
                .await;

                relay_logic_calls(
                    &current_valset,
                    ethereum_key,
                    &web3,
                    &mut grpc_client,
                    gravity_contract_address,
                    gravity_id.clone(),
                    LOOP_SPEED,
                    relayer_config,
                )
                .await;

                Ok(())
            },
            sleep(LOOP_SPEED)
        );

        if let Err(e) = async_result {
            return Err(e);
        }
    }
}
