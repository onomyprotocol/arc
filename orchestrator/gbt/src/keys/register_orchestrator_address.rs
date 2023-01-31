use cosmos_gravity::send::set_gravity_delegate_addresses;
use gravity_utils::{
    connection_prep::{check_for_fee, create_rpc_connections, wait_for_cosmos_node_ready},
    error::GravityError,
};

use crate::{args::RegisterOrchestratorAddressOpts, utils::TIMEOUT};

pub async fn register_orchestrator_address(
    args: RegisterOrchestratorAddressOpts,
    prefix: String,
) -> Result<(), GravityError> {
    let fee = args.fees;
    let cosmos_grpc = args.cosmos_grpc;
    let validator_key = args.validator_phrase;
    let ethereum_key = args.ethereum_key;
    let cosmos_key = args.cosmos_phrase;

    let connections = create_rpc_connections(prefix, Some(cosmos_grpc), None, TIMEOUT).await;
    let contact = connections.contact.unwrap();
    wait_for_cosmos_node_ready(&contact).await;

    let validator_addr = validator_key
        .to_address(&contact.get_prefix())
        .expect("Failed to parse validator-phrase");

    check_for_fee(&fee, validator_addr, &contact).await?;

    let ethereum_address = ethereum_key.to_address();
    let cosmos_address = cosmos_key.to_address(&contact.get_prefix()).unwrap();
    let res = set_gravity_delegate_addresses(
        &contact,
        ethereum_address,
        cosmos_address,
        validator_key,
        fee.clone(),
    )
    .await
    .expect("Failed to update Eth address");
    let res = contact.wait_for_tx(res, TIMEOUT).await;

    if let Err(e) = res {
        return Err(GravityError::UnrecoverableError(
            format!(
                "Failed trying to register delegate addresses error {e:?}, correct the error and try again"
            )
        ));
    }

    let eth_address = ethereum_key.to_address();
    info!(
        "Registered Delegate Ethereum address {} and Cosmos address {}",
        eth_address, cosmos_address
    );

    Ok(())
}
