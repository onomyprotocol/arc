//! Ethereum Event watcher watches for events such as a deposit to the Gravity Ethereum contract or a validator set update
//! or a transaction batch update. It then responds to these events by performing actions on the Cosmos chain if required

use clarity::{utils::bytes_to_hex_str, Address as EthAddress, Uint256};
use cosmos_gravity::{query::get_last_event_nonce_for_validator, send::send_ethereum_claims};
use deep_space::Contact;
use deep_space::{coin::Coin, private_key::PrivateKey as CosmosPrivateKey};
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::get_with_retry::get_block_number_with_retry;
use gravity_utils::get_with_retry::get_net_version_with_retry;
use gravity_utils::types::event_signatures::*;
use gravity_utils::{
    error::GravityError,
    types::{
        Erc20DeployedEvent, LogicCallExecutedEvent, SendToCosmosEvent,
        TransactionBatchExecutedEvent, ValsetUpdatedEvent,
    },
};
use tonic::transport::Channel;
use web30::client::Web3;
use web30::jsonrpc::error::Web3Error;

const BLOCK_DELAY: u8 = 35;
// network IDs
const ETHEREUM_MAINNET_ID: u64 = 1;
const ROPSTEN_NET_ID: u64 = 3;
const KOTTI_NET_ID: u64 = 6;
const MORDOR_NET_ID: u64 = 7;
const GRAVITY_TEST_NET_ID: u64 = 15;
const HARDHAT_NET_ID: u64 = 31337;
const RINKEBY_NET_ID: u64 = 4;
const GOERLI_NET_ID: u64 = 5;

pub async fn check_for_events(
    web3: &Web3,
    contact: &Contact,
    grpc_client: &mut GravityQueryClient<Channel>,
    gravity_contract_address: EthAddress,
    our_private_key: CosmosPrivateKey,
    fee: Coin,
    starting_block: Uint256,
) -> Result<Uint256, GravityError> {
    let our_cosmos_address = our_private_key.to_address(&contact.get_prefix()).unwrap();
    let latest_block = get_block_number_with_retry(web3).await;
    let latest_block = latest_block - get_block_delay(web3).await;

    let deposits = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![gravity_contract_address],
            vec![SENT_TO_COSMOS_EVENT_SIG],
        )
        .await;
    trace!("Deposits {:?}", deposits);

    let batches = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![gravity_contract_address],
            vec![TRANSACTION_BATCH_EXECUTED_EVENT_SIG],
        )
        .await;
    trace!("Batches {:?}", batches);

    let valsets = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![gravity_contract_address],
            vec![VALSET_UPDATED_EVENT_SIG],
        )
        .await;
    trace!("Valsets {:?}", valsets);

    let erc20_deployed = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
            vec![gravity_contract_address],
            vec![ERC20_DEPLOYED_EVENT_SIG],
        )
        .await;
    trace!("ERC20 Deployments {:?}", erc20_deployed);

    let logic_call_executed = web3
        .check_for_events(
            starting_block.clone(),
            Some(latest_block.clone()),
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
                "Oracle observed deposit with sender {}, destination {}, amount {}, and event nonce {}",
                deposits[0].sender, deposits[0].destination.to_bech32(contact.get_prefix()).unwrap(), deposits[0].amount, deposits[0].event_nonce
            )
        }
        if !withdraws.is_empty() {
            info!(
                "Oracle observed batch with nonce {}, contract {}, and event nonce {}",
                withdraws[0].batch_nonce, withdraws[0].erc20, withdraws[0].event_nonce
            )
        }
        if !erc20_deploys.is_empty() {
            info!(
                "Oracle observed ERC20 deployment with denom {} erc20 name {} and symbol {} and event nonce {}",
                erc20_deploys[0].cosmos_denom, erc20_deploys[0].name, erc20_deploys[0].symbol, erc20_deploys[0].event_nonce,
            )
        }
        if !logic_calls.is_empty() {
            info!(
                "Oracle observed logic call execution with ID {} Nonce {} and event nonce {}",
                bytes_to_hex_str(&logic_calls[0].invalidation_id),
                logic_calls[0].invalidation_nonce,
                logic_calls[0].event_nonce
            )
        }

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
        Ok(latest_block)
    } else {
        error!("Failed to get events");
        Err(GravityError::RpcError(Box::new(Web3Error::BadResponse(
            "Failed to get logs!".into(),
        ))))
    }
}

/// The number of blocks behind the 'latest block' on Ethereum our event checking should be.
/// Ethereum does not have finality and as such is subject to chain reorgs and temporary forks
/// if we check for events up to the very latest block we may process an event which did not
/// 'actually occur' in the longest POW chain.
///
/// Obviously we must chose some delay in order to prevent incorrect events from being claimed
///
/// For EVM chains with finality the correct value for this is zero. As there's no need
/// to concern ourselves with re-orgs or forking. This function checks the netID of the
/// provided Ethereum RPC and adjusts the block delay accordingly
///
/// We have chosen to go with block delay of 35, giving preference to security over speed.
/// This function has previously discriminated different networks,
/// but now we added the same block delay for all of the networks.
/// We have kept the different networks and their names - in case of future changes.
pub async fn get_block_delay(web3: &Web3) -> Uint256 {
    let net_version = get_net_version_with_retry(web3).await;

    match net_version {
        // all PoW Chains
        ETHEREUM_MAINNET_ID | ROPSTEN_NET_ID | KOTTI_NET_ID | MORDOR_NET_ID => BLOCK_DELAY.into(),
        // Dev, our own Gravity Ethereum testnet, and Hardhat respectively
        // all single signer chains with no chance of any reorgs
        // yet for testing purposes we add the same delay as for the mainnets
        2018 | GRAVITY_TEST_NET_ID | HARDHAT_NET_ID => BLOCK_DELAY.into(),
        // Rinkeby and Goerli use Clique (PoA) Consensus, finality takes
        // up to num validators blocks.
        RINKEBY_NET_ID | GOERLI_NET_ID => BLOCK_DELAY.into(),
        // assume the safe option (POW) where we don't know
        _ => BLOCK_DELAY.into(),
    }
}
