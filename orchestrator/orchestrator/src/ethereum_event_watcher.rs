//! Ethereum Event watcher watches for events such as a deposit to the Gravity Ethereum contract or a validator set update
//! or a transaction batch update. It then responds to these events by performing actions on the Cosmos chain if required

use cosmos_gravity::{query::get_last_event_nonce_for_validator, send::send_ethereum_claims};
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::{
    clarity::{utils::bytes_to_hex_str, Address as EthAddress, Uint256},
    deep_space::{coin::Coin, private_key::PrivateKey as CosmosPrivateKey, Contact},
    error::GravityError,
    get_block_delay, get_expected_block_delay,
    get_with_retry::{get_finalized_block_number_with_retry, get_latest_block_number_with_retry},
    types::{
        event_signatures::*, Erc20DeployedEvent, LogicCallExecutedEvent, SendToCosmosEvent,
        TransactionBatchExecutedEvent, ValsetUpdatedEvent,
    },
    web30::{client::Web3, jsonrpc::error::Web3Error},
    USE_FINALIZATION,
};
use metrics_exporter::metrics_errors_counter;
use tonic::transport::Channel;

#[derive(Clone, Copy)]
pub struct CheckedNonces {
    pub block_number: Uint256,
    pub event_nonce: Uint256,
}

#[allow(clippy::too_many_arguments)]
pub async fn check_for_events(
    web3: &Web3,
    contact: &Contact,
    grpc_client: &mut GravityQueryClient<Channel>,
    gravity_contract_address: EthAddress,
    our_private_key: CosmosPrivateKey,
    fee: Coin,
    starting_block: Uint256,
) -> Result<CheckedNonces, GravityError> {
    let our_cosmos_address = our_private_key.to_address(&contact.get_prefix()).unwrap();

    let ending_block = if USE_FINALIZATION {
        // get this first in case inbetween the calls is a block boundary
        // don't accidentally use this variable elswhere
        let unsafe_latest_block = get_latest_block_number_with_retry(web3).await;

        // NOTE: the delay can only be omitted if we are using the `finalized` version on a PoS network
        let finalized_block = get_finalized_block_number_with_retry(web3).await;

        let expected_delay = get_expected_block_delay(web3).await;

        // do this even if `expected_delay` is zero, be extra paranoid
        if finalized_block.checked_add(expected_delay).unwrap() > unsafe_latest_block {
            return Err(GravityError::UnrecoverableError(format!(
                "the finalized block number ({:?}) does not have the expected minimum delay \
                ({:?}) over the latest block number ({:?})",
                finalized_block, expected_delay, unsafe_latest_block
            )));
        }

        finalized_block
    } else {
        let latest_block = get_latest_block_number_with_retry(web3).await;
        latest_block
            .checked_sub(get_block_delay(web3).await)
            .ok_or_else(|| {
                GravityError::UnrecoverableError(
                    // This should only happen if the bridge is started immediately after the chain
                    // genesis which will not happen in production. In tests this is an indicator that
                    // `get_block_delay` is not setting the delay to zero for the testnet id.
                    "Latest block number is less than the block delay".to_owned(),
                )
            })?
    };

    let deposits = web3
        .check_for_events(
            starting_block,
            Some(ending_block),
            vec![gravity_contract_address],
            vec![SENT_TO_COSMOS_EVENT_SIG],
        )
        .await;
    trace!("Deposits {:?}", deposits);

    let batches = web3
        .check_for_events(
            starting_block,
            Some(ending_block),
            vec![gravity_contract_address],
            vec![TRANSACTION_BATCH_EXECUTED_EVENT_SIG],
        )
        .await;
    trace!("Batches {:?}", batches);

    let valsets = web3
        .check_for_events(
            starting_block,
            Some(ending_block),
            vec![gravity_contract_address],
            vec![VALSET_UPDATED_EVENT_SIG],
        )
        .await;
    trace!("Valsets {:?}", valsets);

    let erc20_deployed = web3
        .check_for_events(
            starting_block,
            Some(ending_block),
            vec![gravity_contract_address],
            vec![ERC20_DEPLOYED_EVENT_SIG],
        )
        .await;
    trace!("ERC20 Deployments {:?}", erc20_deployed);

    let logic_call_executed = web3
        .check_for_events(
            starting_block,
            Some(ending_block),
            vec![gravity_contract_address],
            vec![LOGIC_CALL_EVENT_SIG],
        )
        .await;
    trace!("Logic call executions {:?}", logic_call_executed);

    if let (Ok(valsets), Ok(batches), Ok(deposits), Ok(deploys), Ok(logic_calls)) = (
        valsets,
        batches,
        deposits,
        erc20_deployed,
        logic_call_executed,
    ) {
        let valsets = ValsetUpdatedEvent::from_logs(&valsets)?;
        trace!("parsed valsets {:?}", valsets);
        let withdraws = TransactionBatchExecutedEvent::from_logs(&batches)?;
        trace!("parsed batches {:?}", batches);
        let deposits = SendToCosmosEvent::from_logs(&deposits)?;
        trace!("parsed deposits {:?}", deposits);
        let erc20_deploys = Erc20DeployedEvent::from_logs(&deploys)?;
        trace!("parsed erc20 deploys {:?}", erc20_deploys);
        let logic_calls = LogicCallExecutedEvent::from_logs(&logic_calls)?;
        trace!("logic call executions {:?}", logic_calls);

        // note that starting block overlaps with our last checked block, because we have to deal with
        // the possibility that the relayer was killed after relaying only one of multiple events in a single
        // block, so we also need this routine so make sure we don't send in the first event in this hypothetical
        // multi event block again. In theory we only send all events for every block and that will pass of fail
        // atomicly but lets not take that risk.
        let last_event_nonce = get_last_event_nonce_for_validator(
            grpc_client,
            our_cosmos_address,
            contact.get_prefix(),
        )
        .await?;
        let valsets = ValsetUpdatedEvent::filter_by_event_nonce(last_event_nonce, &valsets);
        let deposits = SendToCosmosEvent::filter_by_event_nonce(last_event_nonce, &deposits);
        let withdraws =
            TransactionBatchExecutedEvent::filter_by_event_nonce(last_event_nonce, &withdraws);
        let erc20_deploys =
            Erc20DeployedEvent::filter_by_event_nonce(last_event_nonce, &erc20_deploys);
        let logic_calls =
            LogicCallExecutedEvent::filter_by_event_nonce(last_event_nonce, &logic_calls);

        if !valsets.is_empty() {
            info!(
                "Oracle observed Valset update with nonce {} and event nonce {}",
                valsets[0].valset_nonce, valsets[0].event_nonce
            )
        }
        if !deposits.is_empty() {
            info!(
                "Oracle observed deposit with sender {}, destination {:?}, amount {}, and event nonce {}",
                deposits[0].sender, deposits[0].validated_destination, deposits[0].amount, deposits[0].event_nonce
            )
        }
        if !withdraws.is_empty() {
            info!(
                "Oracle observed batch with nonce {}, contract {}, and event nonce {}",
                withdraws[0].batch_nonce, withdraws[0].erc20, withdraws[0].event_nonce
            )
        }
        if !erc20_deploys.is_empty() {
            let v = erc20_deploys[0].clone();
            if v.cosmos_denom.len() < 1000 && v.name.len() < 1000 && v.symbol.len() < 1000 {
                info!(
                "Oracle observed ERC20 deployment with denom {} erc20 name {} and symbol {} and event nonce {}",
                erc20_deploys[0].cosmos_denom, erc20_deploys[0].name, erc20_deploys[0].symbol, erc20_deploys[0].event_nonce,
                );
            } else {
                info!(
                    "Oracle observed ERC20 deployment with  event nonce {}",
                    erc20_deploys[0].event_nonce,
                );
            }
        }
        if !logic_calls.is_empty() {
            info!(
                "Oracle observed logic call execution with ID {} Nonce {} and event nonce {}",
                bytes_to_hex_str(&logic_calls[0].invalidation_id),
                logic_calls[0].invalidation_nonce,
                logic_calls[0].event_nonce
            )
        }

        let new_event_nonce = Uint256::from_u64(last_event_nonce);
        if !deposits.is_empty()
            || !withdraws.is_empty()
            || !erc20_deploys.is_empty()
            || !logic_calls.is_empty()
            || !valsets.is_empty()
        {
            let res = send_ethereum_claims(
                contact,
                our_private_key,
                deposits,
                withdraws,
                erc20_deploys,
                logic_calls,
                valsets,
                fee,
            )
            .await?;

            let new_event_nonce = get_last_event_nonce_for_validator(
                grpc_client,
                our_cosmos_address,
                contact.get_prefix(),
            )
            .await?;

            info!("Current event nonce is {}", new_event_nonce);

            // since we can't actually trust that the above txresponse is correct we have to check here
            // we may be able to trust the tx response post grpc
            if new_event_nonce == last_event_nonce {
                return Err(GravityError::ValidationError(
                    format!("Claims did not process, trying to update but still on {}, trying again in a moment, check txhash {} for errors", last_event_nonce, res.txhash),
                ));
            } else {
                info!("Claims processed, new nonce {}", new_event_nonce);
            }
        }
        Ok(CheckedNonces {
            block_number: ending_block,
            event_nonce: new_event_nonce,
        })
    } else {
        error!("Failed to get events");
        metrics_errors_counter(1, "Failed to get events");
        Err(GravityError::RpcError(Box::new(Web3Error::BadResponse(
            "Failed to get logs!".into(),
        ))))
    }
}
