//! The Gravity deployERC20 endpoint deploys an ERC20 contract representing a Cosmos asset onto the Ethereum blockchain
//! the event for this deployment is then ferried over to Cosmos where the validators will accept the ERC20 contract address
//! as the representation of this asset on Ethereum

use std::time::Duration;

use gravity_utils::{
    clarity::{
        abi::{encode_call, Token},
        u256, Address, PrivateKey, Uint256,
    },
    error::GravityError,
    u64_array_bigints,
    web30::{client::Web3, types::SendTxOption},
};

/// Calls the Gravity ethereum contract to deploy the ERC20 representation of the given Cosmos asset
/// denom. If an existing contract is already deployed representing this asset this call will cost
/// Gas but not actually do anything. Returns the txhash or an error
#[allow(clippy::too_many_arguments)]
pub async fn deploy_erc20(
    cosmos_denom: String,
    erc20_name: String,
    erc20_symbol: String,
    decimals: u32,
    gravity_contract: Address,
    web3: &Web3,
    wait_timeout: Option<Duration>,
    sender_secret: PrivateKey,
    options: Vec<SendTxOption>,
) -> Result<Uint256, GravityError> {
    let sender_address = sender_secret.to_address();
    let tx_hash = web3
        .send_transaction(
            gravity_contract,
            encode_call(
                "deployERC20(string,string,string,uint8)",
                &[
                    Token::String(cosmos_denom),
                    Token::String(erc20_name),
                    Token::String(erc20_symbol),
                    decimals.into(),
                ],
            )?,
            u256!(0),
            sender_address,
            &sender_secret,
            options,
        )
        .await?;

    if let Some(timeout) = wait_timeout {
        web3.wait_for_transaction(tx_hash, timeout, None).await?;
    }

    Ok(tx_hash)
}
