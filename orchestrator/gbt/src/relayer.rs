use std::path::Path;

use cosmos_gravity::query::get_gravity_params;
use gravity_utils::{
    clarity::constants::ZERO_ADDRESS,
    connection_prep::{
        check_for_eth, check_for_fee, create_rpc_connections, wait_for_cosmos_node_ready,
    },
    deep_space::PrivateKey as CosmosPrivateKey,
    error::GravityError,
    types::{BatchRequestMode, RelayerConfig},
};
use relayer::main_loop::{relayer_main_loop, TIMEOUT};

use crate::{
    args::RelayerOpts,
    config::{config_exists, load_keys},
    utils::print_relaying_explanation,
};

pub async fn relayer(
    args: RelayerOpts,
    address_prefix: String,
    home_dir: &Path,
    config: &RelayerConfig,
) -> Result<(), GravityError> {
    let cosmos_grpc = args.cosmos_grpc;
    let ethereum_rpc = args.ethereum_rpc;
    let ethereum_key = args.ethereum_key;

    let cosmos_key = args.cosmos_phrase;
    let connections = create_rpc_connections(
        address_prefix,
        Some(cosmos_grpc),
        Some(ethereum_rpc),
        TIMEOUT,
    )
    .await;

    let ethereum_key = if let Some(k) = ethereum_key {
        k
    } else {
        let mut k = None;
        if config_exists(home_dir) {
            let keys = load_keys(home_dir)?;
            if let Some(stored_key) = keys.ethereum_key {
                k = Some(stored_key)
            }
        }
        if k.is_none() {
            error!("You must specify an Ethereum key!");
            error!("To generate, register, and store a key use `gbt keys register-orchestrator-address`");
            error!("Store an already registered key using 'gbt keys set-ethereum-key`");
            error!("To run from the command line, with no key storage use 'gbt relayer --ethereum-key your key' ");
            return Err(GravityError::UnrecoverableError(
                "Ethereum key not specified".into(),
            ));
        }
        k.unwrap()
    };
    let cosmos_key = if let Some(k) = cosmos_key {
        Some(k)
    } else if config_exists(home_dir) {
        let keys = load_keys(home_dir)?;
        keys.orchestrator_phrase
            .map(|stored_key| CosmosPrivateKey::from_phrase(&stored_key, "").unwrap())
    } else {
        None
    };

    let public_eth_key = ethereum_key.to_address();
    info!("Starting Gravity Relayer");
    info!("Ethereum Address: {}", public_eth_key);

    let contact = connections.contact.clone().unwrap();
    let web3 = connections.web3.unwrap();
    let mut grpc = connections.grpc.unwrap();

    // check if the cosmos node is syncing, if so wait for it
    // we can't move any steps above this because they may fail on an incorrect
    // historic chain state while syncing occurs
    wait_for_cosmos_node_ready(&contact).await;
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
                if v == *ZERO_ADDRESS {
                    return Err(GravityError::UnrecoverableError(
                        "The Gravity address is not yet set as a chain parameter! You must specify --gravity-contract-address".into(),
                    ));
                }

                v
            }
            Err(_) => {
                return Err(GravityError::UnrecoverableError("The Gravity address is not yet set as a chain parameter! You must specify --gravity-contract-address".into()));
            }
        }
    };
    info!("Gravity contract address {}", contract_address);

    // setup and explain relayer settings
    if let (Some(fee), Some(cosmos_key)) = (args.fees.clone(), cosmos_key) {
        if config.batch_request_mode != BatchRequestMode::None {
            let public_cosmos_key = cosmos_key.to_address(&contact.get_prefix()).unwrap();
            check_for_fee(&fee, public_cosmos_key, &contact).await?;
            print_relaying_explanation(config, true)
        } else {
            print_relaying_explanation(config, false)
        }
    } else {
        print_relaying_explanation(config, false)
    }

    relayer_main_loop(
        ethereum_key,
        cosmos_key,
        args.fees,
        web3,
        contact,
        grpc,
        contract_address,
        params.gravity_id,
        config,
    )
    .await
}
