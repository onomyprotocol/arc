use ethereum_gravity::{send_to_cosmos::send_to_cosmos, utils::get_valset_nonce};
use gravity_utils::{
    connection_prep::{check_for_eth, create_rpc_connections},
    error::GravityError,
    num_conversion::fraction_to_exponent,
};

use crate::{args::EthToCosmosOpts, utils::TIMEOUT};

pub async fn eth_to_cosmos(args: EthToCosmosOpts, prefix: String) -> Result<(), GravityError> {
    let gravity_address = args.gravity_contract_address;
    let erc20_address = args.token_contract_address;
    let cosmos_dest = args.destination;
    let ethereum_key = args.ethereum_key;
    let ethereum_public_key = ethereum_key.to_address();
    let ethereum_rpc = args.ethereum_rpc;
    let amount = args.amount;

    let connections = create_rpc_connections(prefix, None, Some(ethereum_rpc), TIMEOUT).await;

    let web3 = connections.web3.unwrap();

    get_valset_nonce(gravity_address, ethereum_public_key, &web3)
        .await
        .expect("Incorrect Gravity Address or otherwise unable to contact Gravity");

    check_for_eth(ethereum_public_key, &web3).await?;

    let res = web3
        .get_erc20_decimals(erc20_address, ethereum_public_key)
        .await
        .expect("Failed to query ERC20 contract");
    let decimals: u8 = res.to_string().parse().unwrap();
    let amount = fraction_to_exponent(amount, decimals);

    let erc20_balance = web3
        .get_erc20_balance(erc20_address, ethereum_public_key)
        .await
        .expect("Failed to get balance, check ERC20 contract address");

    if erc20_balance.is_zero() {
        return Err(GravityError::UnrecoverableError(format!(
            "You have zero {erc20_address} tokens, please double check your sender and erc20 addresses!"
        )));
    } else if amount > erc20_balance {
        return Err(GravityError::UnrecoverableError(format!(
            "Insufficient balance {amount} > {erc20_balance}"
        )));
    }

    info!(
        "Sending {} / {} to Cosmos from {} to {}",
        amount, erc20_address, ethereum_public_key, cosmos_dest
    );
    // we send some erc20 tokens to the gravity contract to register a deposit
    let res = send_to_cosmos(
        erc20_address,
        gravity_address,
        amount,
        cosmos_dest,
        ethereum_key,
        TIMEOUT,
        &web3,
        vec![],
    )
    .await;
    match res {
        Ok(tx_id) => info!("Send to Cosmos txid: {:#066x}", tx_id),
        Err(e) => {
            return Err(GravityError::UnrecoverableError(format!(
                "Failed to send tokens! {e:?}"
            )))
        }
    }
    info!(
        "Your tokens should show up in the account {} on Gravity Bridge within 10 minutes",
        cosmos_dest
    );
    Ok(())
}
