//! This file handles the automatic request of batches, see the documentation on batch creation
//! https://github.com/onomyprotocol/cosmos-gravity-bridge/blob/main/spec/batch-creation-spec.md
//! By having batches requested by relayers instead of created automatically the chain can outsource
//! the significant work of checking if a batch is profitable before creating it

use cosmos_gravity::{
    query::{get_erc20_to_denom, get_pending_batch_fees},
    send::send_request_batch,
};
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::{
    clarity::Address as EthAddress,
    deep_space::{Coin, Contact, PrivateKey},
    types::BatchRequestMode,
    web30::client::Web3,
};
use tonic::transport::Channel;

pub async fn request_batches(
    contact: &Contact,
    web30: &Web3,
    grpc_client: &mut GravityQueryClient<Channel>,
    batch_request_mode: BatchRequestMode,
    private_key: PrivateKey,
    request_fee: Coin,
) {
    // this actually works either way but sending a tx with zero as the fee
    // value seems strange
    let request_fee = if request_fee.amount.is_zero() {
        None
    } else {
        Some(request_fee)
    };
    // get the gas price once
    let eth_gas_price = web30.eth_gas_price().await;
    if let Err(e) = eth_gas_price {
        warn!("Could not get gas price for auto batch request {:?}", e);
        return;
    }

    let batch_fees = get_pending_batch_fees(grpc_client).await;
    if let Err(e) = batch_fees {
        warn!("Failed to get batch fees with {:?}", e);
        return;
    }
    let batch_fees = batch_fees.unwrap();

    for fee in batch_fees.batch_fees {
        let token: EthAddress = fee.token.parse().unwrap();
        let denom = get_erc20_to_denom(grpc_client, token).await;
        if let Err(e) = denom {
            error!(
                "Failed to lookup erc20 {} for batch with {:?}",
                fee.token, e
            );
            continue;
        }
        let denom = denom.unwrap().denom;

        match batch_request_mode {
            BatchRequestMode::EveryBatch => {
                info!("Requesting batch for {}", fee.token);
                let res =
                    send_request_batch(private_key, denom, request_fee.clone(), contact).await;
                if let Err(e) = res {
                    warn!("Failed to request batch with {:?}", e);
                }
            }
            BatchRequestMode::None => {}
        }
    }
}
