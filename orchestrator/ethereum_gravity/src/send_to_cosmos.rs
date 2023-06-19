//! Helper functions for sending tokens to Cosmos

use std::time::Duration;

use gravity_utils::{
    clarity::{
        abi::{encode_call, Token},
        u256, Address, PrivateKey as EthPrivateKey, Uint256,
    },
    deep_space::address::Address as CosmosAddress,
    error::GravityError,
    u64_array_bigints,
    web30::{client::Web3, types::SendTxOption},
};

#[allow(clippy::too_many_arguments)]
pub async fn send_to_cosmos(
    erc20: Address,
    gravity_contract: Address,
    amount: Uint256,
    cosmos_destination: CosmosAddress,
    sender_secret: EthPrivateKey,
    wait_timeout: Duration,
    web3: &Web3,
    options: Vec<SendTxOption>,
) -> Result<Uint256, GravityError> {
    let sender_address = sender_secret.to_address();

    // if the user sets a gas limit we should honor it, if they don't we
    // should add the default
    let mut options = options;
    let mut has_gas_limit = false;
    for option in options.iter() {
        if let SendTxOption::Nonce(_) = option {
            return Err(GravityError::ValidationError(
                "This call sends more than one tx! Can't specify".into(),
            ));
        }
        if let SendTxOption::GasLimit(_) = option {
            has_gas_limit = true;
        }
    }

    if !has_gas_limit {
        options.push(SendTxOption::GasLimit(web3.eth_gas_price().await?));
    }

    // add nonce to options
    let nonce = web3.eth_get_transaction_count(sender_address).await?;
    options.push(SendTxOption::Nonce(nonce));

    // rapidly changing gas prices can cause this to fail, a quick retry loop here
    // retries in a way that assists our transaction stress test
    let check_and_approve_erc20_transfer = async {
        loop {
            if let Ok(approved) = web3
                .check_erc20_approved(erc20, sender_address, gravity_contract)
                .await
            {
                if !approved {
                    info!(
                        "Approving MAX {} from {} for gravity contract",
                        erc20, sender_address
                    );
                    let txid = web3
                        .approve_erc20_transfers(
                            erc20,
                            &sender_secret,
                            gravity_contract,
                            None,
                            options.clone(),
                        )
                        .await
                        .expect("Can't approve erc20 transfers within timeout");
                    debug!(
                        "We are not approved for ERC20 transfers, approving txid: {:#066x}",
                        txid
                    );
                    web3.wait_for_transaction(txid, wait_timeout, None)
                        .await
                        .unwrap_or_else(|_| {
                            panic!(
                                "Can't await for transaction within timeout, txid: {:#066x}",
                                txid
                            )
                        });
                    // increment the nonce for the next call
                    options.push(SendTxOption::Nonce(nonce.checked_add(u256!(1)).unwrap()));
                }
                break;
            }
        }
    };

    if tokio::time::timeout(wait_timeout, check_and_approve_erc20_transfer)
        .await
        .is_err()
    {
        return Err(GravityError::UnrecoverableError(
            "Can't check and approve erc20 transfer within timeout".into(),
        ));
    }

    info!(
        "Sending {}{} from {} to cosmos {}",
        amount, erc20, sender_address, cosmos_destination
    );
    let encoded_destination_address = Token::String(cosmos_destination.to_string());

    let tx_hash = web3
        .send_transaction(
            gravity_contract,
            encode_call(
                "sendToCosmos(address,string,uint256)",
                &[erc20.into(), encoded_destination_address, amount.into()],
            )?,
            u256!(0),
            sender_address,
            &sender_secret,
            options,
        )
        .await?;

    web3.wait_for_transaction(tx_hash, wait_timeout, None)
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Can't await for transaction within timeout, txid: {:#066x}",
                tx_hash
            )
        });

    Ok(tx_hash)
}
