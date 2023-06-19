use std::{cmp::min, time::Duration};

use cosmos_gravity::query::get_gravity_params;
use gravity_utils::{
    clarity::constants::ZERO_ADDRESS,
    connection_prep::{
        check_delegate_addresses, check_for_eth, check_for_fee, create_rpc_connections,
        wait_for_cosmos_node_ready,
    },
    error::GravityError,
    get_block_delay,
    get_with_retry::get_net_version_with_retry,
    types::{BatchRequestMode, GravityBridgeToolsConfig},
    TEST_ETH_CHAIN_ID, USE_FINALIZATION,
};
use metrics_exporter::metrics_server;
use orchestrator::main_loop::{
    orchestrator_main_loop, ETH_ORACLE_LOOP_SPEED, ETH_SIGNER_LOOP_SPEED,
};

use crate::{args::OrchestratorOpts, utils::print_relaying_explanation};

pub async fn orchestrator(
    args: OrchestratorOpts,
    address_prefix: String,
    config: GravityBridgeToolsConfig,
) -> Result<(), GravityError> {
    let fee = args.fees;
    let cosmos_grpc = args.cosmos_grpc;
    let ethereum_rpc = args.ethereum_rpc;
    let ethereum_key = args.ethereum_key;
    let cosmos_key = args.cosmos_phrase;

    let timeout = min(
        min(ETH_SIGNER_LOOP_SPEED, ETH_ORACLE_LOOP_SPEED),
        Duration::from_secs(config.relayer.relayer_loop_speed),
    );

    trace!("Probing RPC connections");
    // probe all rpc connections and see if they are valid
    let connections = create_rpc_connections(
        address_prefix,
        Some(cosmos_grpc),
        Some(ethereum_rpc),
        timeout,
    )
    .await;

    let mut grpc = connections.grpc.clone().unwrap();
    let contact = connections.contact.clone().unwrap();
    let web3 = connections.web3.clone().unwrap();

    let public_eth_key = ethereum_key.to_address();
    let public_cosmos_key = cosmos_key
        .to_address(&contact.get_prefix())
        .expect("Failed to parse cosmos-phrase");
    info!("Starting Gravity Validator companion binary Relayer + Oracle + Eth Signer");
    info!(
        "Ethereum Address: {} Cosmos Address {}",
        public_eth_key, public_cosmos_key
    );

    // so we can double check in the logs that there is no configuration problem
    let net_version = get_net_version_with_retry(&web3).await;
    let block_delay = get_block_delay(&web3).await;
    info!("Chain ID is {}", net_version);
    if net_version == TEST_ETH_CHAIN_ID {
        warn!("Chain ID is equal to TEST_ETH_CHAIN_ID, assuming this is a local test net");
    }
    if USE_FINALIZATION {
        info!("Using finalization for block delays",);
    } else {
        info!(
            "Using probabilistic finality with block delay {}",
            block_delay
        );
    }

    // check if the cosmos node is syncing, if so wait for it
    // we can't move any steps above this because they may fail on an incorrect
    // historic chain state while syncing occurs
    wait_for_cosmos_node_ready(&contact).await;

    // check if the delegate addresses are correctly configured
    check_delegate_addresses(
        &mut grpc,
        public_eth_key,
        public_cosmos_key,
        &contact.get_prefix(),
    )
    .await?;

    // check if we actually have the promised balance of tokens to pay fees
    check_for_fee(&fee, public_cosmos_key, &contact).await?;
    check_for_eth(public_eth_key, &web3).await?;

    // get the gravity parameters
    let params = get_gravity_params(&mut grpc)
        .await
        .expect("Failed to get Gravity Bridge module parameters!");

    // get the gravity contract address, if not provided
    let contract_address = if let Some(c) = args.gravity_contract_address {
        c
    } else {
        let c = params.bridge_ethereum_address.parse();
        match c {
            Ok(v) => {
                if v == ZERO_ADDRESS {
                    return Err(GravityError::UnrecoverableError(
                        "The Gravity address is not yet set as a chain parameter! You must specify --gravity-contract-address".into(),
                    ));
                }
                c.unwrap()
            }
            Err(_) => {
                return Err(GravityError::UnrecoverableError(
                    "The Gravity address is not yet set as a chain parameter! You must specify --gravity-contract-address".into(),
                ));
            }
        }
    };

    if config.orchestrator.relayer_enabled {
        // setup and explain relayer settings
        if config.relayer.batch_request_mode != BatchRequestMode::None {
            check_for_fee(&fee, public_cosmos_key, &contact).await?;
            print_relaying_explanation(&config.relayer, true)
        } else {
            print_relaying_explanation(&config.relayer, false)
        }
    }

    // Start monitiring if enabled on config.toml
    if config.metrics.metrics_enabled {
        metrics_server(&config.metrics);
    };

    orchestrator_main_loop(
        cosmos_key,
        ethereum_key,
        connections.web3.unwrap(),
        connections.contact.unwrap(),
        connections.grpc.unwrap(),
        contract_address,
        params.gravity_id,
        fee,
        config,
    )
    .await
}
