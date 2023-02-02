use std::{panic, time::Duration};

use cosmos_gravity::{proposals::submit_parameter_change_proposal, query::get_gravity_params};
use ethereum_gravity::utils::get_event_nonce;
use futures::future::join_all;
use gravity_proto::{
    cosmos_sdk_proto::cosmos::{
        bank::v1beta1::Metadata,
        gov::v1beta1::VoteOption,
        params::v1beta1::{ParamChange, ParameterChangeProposal},
        staking::v1beta1::QueryValidatorsRequest,
    },
    gravity::{query_client::QueryClient as GravityQueryClient, MsgSendToCosmosClaim},
};
use gravity_utils::{
    clarity::{u256, Address as EthAddress, PrivateKey as EthPrivateKey, Transaction, Uint256},
    deep_space::{
        address::Address as CosmosAddress, coin::Coin, error::CosmosGrpcError,
        private_key::PrivateKey as CosmosPrivateKey, Contact, Fee, Msg,
    },
    types::{BatchRelayingMode, BatchRequestMode, GravityBridgeToolsConfig, ValsetRelayingMode},
    u64_array_bigints,
    web30::{client::Web3, jsonrpc::error::Web3Error, types::SendTxOption},
    TEST_GAS_LIMIT,
};
use orchestrator::main_loop::orchestrator_main_loop;
use rand::Rng;
use tokio::time::sleep;

use crate::{
    get_deposit, get_fee, ADDRESS_PREFIX, COSMOS_NODE_GRPC, ETH_NODE, MINER_ADDRESS,
    MINER_PRIVATE_KEY, ONE_ETH, ONE_HUNDRED_ETH, OPERATION_TIMEOUT, STAKING_TOKEN, TOTAL_TIMEOUT,
};

/// returns the required denom metadata for deployed the Footoken
/// token defined in our test environment
pub async fn footoken_metadata(contact: &Contact) -> Metadata {
    let metadata = contact.get_all_denoms_metadata().await.unwrap();
    for m in metadata {
        if m.base == "footoken" {
            return m;
        }
    }
    panic!("Footoken metadata not set?");
}

pub fn get_decimals(meta: &Metadata) -> u32 {
    for m in meta.denom_units.iter() {
        if m.denom == meta.display {
            return m.exponent;
        }
    }
    panic!("Invalid metadata!")
}

pub fn create_default_test_config() -> GravityBridgeToolsConfig {
    let mut no_relay_market_config = GravityBridgeToolsConfig::default();
    // enable integrated relayer by default for tests
    no_relay_market_config.orchestrator.relayer_enabled = true;
    no_relay_market_config.relayer.batch_relaying_mode = BatchRelayingMode::EveryBatch;
    no_relay_market_config.relayer.logic_call_market_enabled = false;
    no_relay_market_config.relayer.valset_relaying_mode = ValsetRelayingMode::EveryValset;
    no_relay_market_config.relayer.batch_request_mode = BatchRequestMode::EveryBatch;
    no_relay_market_config.relayer.relayer_loop_speed = 10;
    no_relay_market_config
}

pub fn create_no_batch_requests_config() -> GravityBridgeToolsConfig {
    let mut no_relay_market_config = create_default_test_config();
    no_relay_market_config.relayer.batch_request_mode = BatchRequestMode::None;
    no_relay_market_config
}

pub async fn send_eth_to_orchestrators(keys: &[ValidatorKeys], web30: &Web3) {
    let balance = web30.eth_get_balance(*MINER_ADDRESS).await.unwrap();
    info!(
        "Sending orchestrators 100 eth to pay for fees miner has {} ETH",
        balance.divide(ONE_ETH).unwrap().0
    );
    let mut eth_keys = Vec::new();
    for key in keys {
        eth_keys.push(key.eth_key.to_address());
    }
    send_eth_bulk(ONE_HUNDRED_ETH, &eth_keys, web30).await;
}

pub async fn send_one_eth(dest: EthAddress, web30: &Web3) {
    send_eth_bulk(ONE_ETH, &[dest], web30).await;
}

pub fn get_coins(denom: &str, balances: &[Coin]) -> Option<Coin> {
    for coin in balances {
        if coin.denom.starts_with(denom) {
            return Some(coin.clone());
        }
    }
    None
}

/// This function efficiently distributes ERC20 tokens to a large number of provided Ethereum addresses
/// the real problem here is that you can't do more than one send operation at a time from a
/// single address without your sequence getting out of whack. By manually setting the nonce
/// here we can send thousands of transactions in only a few blocks
pub async fn send_erc20_bulk(
    amount: Uint256,
    erc20: EthAddress,
    destinations: &[EthAddress],
    web3: &Web3,
) {
    check_erc20_balance(erc20, amount, *MINER_ADDRESS, web3).await;
    let mut nonce = web3
        .eth_get_transaction_count(*MINER_ADDRESS)
        .await
        .unwrap();
    let mut transactions = Vec::new();
    for address in destinations {
        let send = web3.erc20_send(
            amount,
            *address,
            erc20,
            *MINER_PRIVATE_KEY,
            Some(OPERATION_TIMEOUT),
            vec![
                SendTxOption::Nonce(nonce),
                SendTxOption::GasLimit(TEST_GAS_LIMIT),
                SendTxOption::GasPriceMultiplier(5.0),
            ],
        );
        transactions.push(send);
        nonce = nonce.checked_add(u256!(1)).unwrap();
    }
    let txids = join_all(transactions).await;
    wait_for_txids(txids, web3).await;
    let mut balance_checks = Vec::new();
    for address in destinations {
        let check = check_erc20_balance(erc20, amount, *address, web3);
        balance_checks.push(check);
    }
    join_all(balance_checks).await;
}

/// This function efficiently distributes ETH to a large number of provided Ethereum addresses
/// the real problem here is that you can't do more than one send operation at a time from a
/// single address without your sequence getting out of whack. By manually setting the nonce
/// here we can quickly send thousands of transactions in only a few blocks
pub async fn send_eth_bulk(amount: Uint256, destinations: &[EthAddress], web3: &Web3) {
    let net_version = web3.net_version().await.unwrap();
    let mut nonce = web3
        .eth_get_transaction_count(*MINER_ADDRESS)
        .await
        .unwrap();
    let mut transactions = Vec::new();
    let gas_price: Uint256 = web3.eth_gas_price().await.unwrap();
    let double = gas_price.checked_mul(u256!(2)).unwrap();
    for address in destinations {
        let t = Transaction {
            to: *address,
            nonce,
            gas_price: double,
            gas_limit: TEST_GAS_LIMIT,
            value: amount,
            data: Vec::new(),
            signature: None,
        };
        let t = t.sign(&MINER_PRIVATE_KEY, Some(net_version));
        transactions.push(t);
        nonce = nonce.checked_add(u256!(1)).unwrap();
    }
    let mut sends = Vec::new();
    for tx in transactions {
        sends.push(web3.eth_send_raw_transaction(tx.to_bytes().unwrap()));
    }
    let txids = join_all(sends).await;
    wait_for_txids(txids, web3).await;
}

/// utility function that waits for a large number of txids to enter a block
async fn wait_for_txids(txids: Vec<Result<Uint256, Web3Error>>, web3: &Web3) {
    let mut wait_for_txid = Vec::new();
    for txid in txids {
        let wait = web3.wait_for_transaction(txid.unwrap(), TOTAL_TIMEOUT, None);
        wait_for_txid.push(wait);
    }
    let results = join_all(wait_for_txid).await;
    for (i, res) in results.into_iter().enumerate() {
        if let Err(e) = res {
            panic!("`wait_for_txids` failed on index {}: {:?}", i, e);
        }
    }
}

/// utility function for bulk checking erc20 balances, used to provide
/// a single future that contains the assert as well as the request
pub async fn check_erc20_balance(
    erc20: EthAddress,
    amount: Uint256,
    address: EthAddress,
    web3: &Web3,
) {
    let new_balance = get_erc20_balance_safe(erc20, web3, address).await;
    let new_balance = new_balance.unwrap();
    assert!(new_balance >= amount);
}

/// utility function for bulk checking erc20 balances, used to provide
/// a single future that contains the assert as well s the request
pub async fn get_erc20_balance_safe(
    erc20: EthAddress,
    web3: &Web3,
    address: EthAddress,
) -> Result<Uint256, Web3Error> {
    let get_erc20_balance = async {
        loop {
            match web3.get_erc20_balance(erc20, address).await {
                Ok(new_balance) => return Some(new_balance),
                Err(err) => {
                    // only keep trying if our error is gas related
                    if !err.to_string().contains("maxFeePerGas") {
                        return None;
                    }
                }
            }
        }
    };

    match tokio::time::timeout(TOTAL_TIMEOUT, get_erc20_balance).await {
        Err(_) => Err(Web3Error::BadInput("Intentional Error".to_string())),
        Ok(new_balance) => Ok(new_balance.unwrap()),
    }
}

pub fn get_user_key() -> BridgeUserKey {
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();
    // the starting location of the funds
    let eth_key = EthPrivateKey::from_slice(&secret).unwrap();
    let eth_address = eth_key.to_address();
    // the destination on cosmos that sends along to the final ethereum destination
    let cosmos_key = CosmosPrivateKey::from_secret(&secret);
    let cosmos_address = cosmos_key.to_address(&ADDRESS_PREFIX).unwrap();
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();
    // the final destination of the tokens back on Ethereum
    let eth_dest_key = EthPrivateKey::from_slice(&secret).unwrap();
    let eth_dest_address = eth_key.to_address();
    BridgeUserKey {
        eth_address,
        eth_key,
        cosmos_address,
        cosmos_key,
        eth_dest_address,
        eth_dest_key,
    }
}
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct BridgeUserKey {
    // the starting addresses that get Eth balances to send across the bridge
    pub eth_address: EthAddress,
    pub eth_key: EthPrivateKey,
    // the cosmos addresses that get the funds and send them on to the dest eth addresses
    pub cosmos_address: CosmosAddress,
    pub cosmos_key: CosmosPrivateKey,
    // the location tokens are sent back to on Ethereum
    pub eth_dest_address: EthAddress,
    pub eth_dest_key: EthPrivateKey,
}

#[derive(Debug, Clone, Copy)]
pub struct ValidatorKeys {
    /// The Ethereum key used by this validator to sign Gravity bridge messages
    pub eth_key: EthPrivateKey,
    /// The Orchestrator key used by this validator to submit oracle messages and signatures
    /// to the cosmos chain
    pub orch_key: CosmosPrivateKey,
    /// The validator key used by this validator to actually sign and produce blocks
    pub validator_key: CosmosPrivateKey,
}

/// This function pays the piper for the strange concurrency model that we use for the tests
/// we spwan a thread, create a tokio executor and then start the orchestrator within that scope
pub async fn start_orchestrators(
    keys: Vec<ValidatorKeys>,
    gravity_address: EthAddress,
    validator_out: bool,
    orchestrator_config: GravityBridgeToolsConfig,
) {
    // used to break out of the loop early to simulate one validator
    // not running an Orchestrator
    let num_validators = keys.len();
    let mut count = 0;

    #[allow(clippy::explicit_counter_loop)]
    for k in keys {
        let config = orchestrator_config.clone();
        info!(
            "Spawning Orchestrator with delegate keys {} {} and validator key {}",
            k.eth_key.to_address(),
            k.orch_key.to_address(&ADDRESS_PREFIX).unwrap(),
            get_operator_address(k.validator_key),
        );
        let mut grpc_client = GravityQueryClient::connect(COSMOS_NODE_GRPC.as_str())
            .await
            .unwrap();
        let params = get_gravity_params(&mut grpc_client)
            .await
            .expect("Failed to get Gravity Bridge module parameters!");

        // but that will execute all the orchestrators in our test in parallel
        // by spawning to tokio's future executor
        drop(tokio::spawn(async move {
            let web30 =
                gravity_utils::web30::client::Web3::new(ETH_NODE.as_str(), OPERATION_TIMEOUT);

            let contact = Contact::new(
                COSMOS_NODE_GRPC.as_str(),
                OPERATION_TIMEOUT,
                ADDRESS_PREFIX.as_str(),
            )
            .unwrap();

            let _ = orchestrator_main_loop(
                k.orch_key,
                k.eth_key,
                web30,
                contact,
                grpc_client,
                gravity_address,
                params.gravity_id,
                get_fee(),
                config,
            )
            .await;
        }));
        // used to break out of the loop early to simulate one validator
        // not running an orchestrator
        count += 1;
        if validator_out && count == num_validators - 1 {
            break;
        }
    }
}

// Submits a false send to cosmos for every orchestrator key in keys, sending amount of erc20_address
// tokens to cosmos_receiver, claiming to come from ethereum_sender for the given fee.
// If a timeout is supplied, contact.send_message() will block waiting for the tx to appear
// Note: These sends to cosmos are false, meaning the ethereum side will have a lower nonce than the
// cosmos side and the bridge will effectively break.
#[allow(clippy::too_many_arguments)]
pub async fn submit_false_claims(
    keys: &[CosmosPrivateKey],
    nonce: u64,
    height: u64,
    amount: Uint256,
    cosmos_receiver: CosmosAddress,
    ethereum_sender: EthAddress,
    erc20_address: EthAddress,
    contact: &Contact,
    fee: &Fee,
    timeout: Option<Duration>,
) {
    for (i, k) in keys.iter().enumerate() {
        let orch_addr = k.to_address(&contact.get_prefix()).unwrap();
        let claim = MsgSendToCosmosClaim {
            event_nonce: nonce,
            block_height: height,
            token_contract: erc20_address.to_string(),
            amount: amount.to_string(),
            cosmos_receiver: cosmos_receiver.to_string(),
            ethereum_sender: ethereum_sender.to_string(),
            orchestrator: orch_addr.to_string(),
        };
        info!("Oracle number {} submitting false deposit {:?}", i, claim);
        let msg_url = "/arcbnb.v1.MsgSendToCosmosClaim";
        let msg = Msg::new(msg_url, claim.clone());
        let res = contact
            .send_message(
                &[msg],
                Some("All your bridge are belong to us".to_string()),
                fee.amount.as_slice(),
                timeout,
                *k,
            )
            .await
            .expect("Failed to submit false claim");
        info!("Oracle {} false claim response {:?}", i, res);
    }
}

/// Creates a proposal to change the params of our test chain
pub async fn create_parameter_change_proposal(
    contact: &Contact,
    key: CosmosPrivateKey,
    params_to_change: Vec<ParamChange>,
) {
    let proposal = ParameterChangeProposal {
        title: "Set gravity settings!".to_string(),
        description: "test proposal".to_string(),
        changes: params_to_change,
    };
    let res = submit_parameter_change_proposal(
        proposal,
        get_deposit(),
        get_fee(),
        contact,
        key,
        Some(TOTAL_TIMEOUT),
    )
    .await
    .unwrap();
    trace!("Gov proposal executed with {:?}", res);
}

/// Gets the operator address for a given validator private key
pub fn get_operator_address(key: CosmosPrivateKey) -> CosmosAddress {
    // this is not guaranteed to be correct, the chain may set the valoper prefix in a
    // different way, but I haven't yet seen one that does not match this pattern
    key.to_address(&format!("{}valoper", *ADDRESS_PREFIX))
        .unwrap()
}

// Prints out current stake to the console
pub async fn print_validator_stake(contact: &Contact) {
    let validators = contact
        .get_validators_list(QueryValidatorsRequest::default())
        .await
        .unwrap();
    for validator in validators {
        info!(
            "Validator {} has {} tokens",
            validator.operator_address, validator.tokens
        );
    }
}

// votes yes on every proposal available
pub async fn vote_yes_on_proposals(
    contact: &Contact,
    keys: &[ValidatorKeys],
    timeout: Option<Duration>,
) {
    let duration = match timeout {
        Some(dur) => dur,
        None => OPERATION_TIMEOUT,
    };
    // Vote yes on all proposals with all validators
    let proposals = contact
        .get_governance_proposals_in_voting_period()
        .await
        .unwrap();
    trace!("Found proposals: {:?}", proposals.proposals);
    for proposal in proposals.proposals {
        for key in keys.iter() {
            let res = contact
                .vote_on_gov_proposal(
                    proposal.proposal_id,
                    VoteOption::Yes,
                    get_fee(),
                    key.validator_key,
                    Some(duration),
                )
                .await
                .unwrap();
            let res = contact.wait_for_tx(res, TOTAL_TIMEOUT).await.unwrap();
            info!(
                "Voting yes on governance proposal costing {} gas",
                res.gas_used
            );
        }
    }
}

// Checks that cosmos_account has each balance specified in expected_cosmos_coins.
// Note: ignores balances not in expected_cosmos_coins
pub async fn check_cosmos_balances(
    contact: &Contact,
    cosmos_account: CosmosAddress,
    expected_cosmos_coins: &[Coin],
) {
    let mut num_found = 0;
    tokio::time::timeout(TOTAL_TIMEOUT, async {
        loop {
            let mut good = true;
            let curr_balances = contact.get_balances(cosmos_account).await.unwrap();
            // These loops use loop labels, see the documentation on loop labels here for more information
            // https://doc.rust-lang.org/reference/expressions/loop-expr.html#loop-labels
            'outer: for bal in curr_balances.iter() {
                if num_found == expected_cosmos_coins.len() {
                    break 'outer; // done searching entirely
                }
                'inner: for j in 0..expected_cosmos_coins.len() {
                    if num_found == expected_cosmos_coins.len() {
                        break 'outer; // done searching entirely
                    }
                    if expected_cosmos_coins[j].denom != bal.denom {
                        continue;
                    }
                    let check = expected_cosmos_coins[j].amount == bal.amount;
                    good = check;
                    if !check {
                        warn!(
                        "found balance {}! expected {} trying again",
                        bal, expected_cosmos_coins[j].amount
                    );
                    }
                    num_found += 1;
                    break 'inner; // done searching for this particular balance
                }
            }

            let check = num_found == curr_balances.len();
            // if it's already false don't set to true
            good = check || good;
            if !check {
                warn!(
                "did not find the correct balance for each expected coin! found {} of {}, trying again",
                num_found,
                curr_balances.len()
            );
            }
            if good {
                return;
            } else {
                sleep(Duration::from_secs(1)).await;
            }
        }
    }).await.expect("Failed to find correct balances in check_cosmos_balances")
}

/// utility function for bulk checking erc20 balances, used to provide
/// a single future that contains the assert as well s the request
pub async fn get_event_nonce_safe(
    gravity_contract_address: EthAddress,
    web3: &Web3,
    caller_address: EthAddress,
) -> Result<u64, Web3Error> {
    tokio::time::timeout(TOTAL_TIMEOUT, async {
        loop {
            let new_balance = get_event_nonce(gravity_contract_address, caller_address, web3).await;
            if let Err(ref e) = new_balance {
                if e.to_string().contains("maxFeePerGas") {
                    continue;
                }
            }
            return new_balance;
        }
    })
    .await
    .expect("Can't get event nonce withing timeout")
}

/// waits for the cosmos chain to start producing blocks, used to prevent race conditions
/// where our tests try to start running before the Cosmos chain is ready
pub async fn wait_for_cosmos_online(contact: &Contact, timeout: Duration) {
    tokio::time::timeout(timeout, async {
        while let Err(CosmosGrpcError::NodeNotSynced) | Err(CosmosGrpcError::ChainNotRunning) =
            contact.wait_for_next_block(timeout).await
        {
            sleep(Duration::from_secs(1)).await;
        }
    })
    .await
    .expect("Cosmos node has not come online during timeout!");
}

/// This function returns the valoper address of a validator
/// to whom delegating the returned amount of staking token will
/// create a 5% or greater change in voting power, triggering the
/// creation of a validator set update.
pub async fn get_validator_to_delegate_to(contact: &Contact) -> (CosmosAddress, Coin) {
    let validators = contact.get_active_validators().await.unwrap();
    let mut total_bonded_stake = u256!(0);
    let mut has_the_least = None;
    let mut lowest = u256!(0);
    for v in validators {
        let amount = Uint256::from_dec_or_hex_str_restricted(&v.tokens).unwrap();
        total_bonded_stake = total_bonded_stake.checked_add(amount).unwrap();

        if lowest.is_zero() || amount < lowest {
            lowest = amount;
            has_the_least = Some(v.operator_address.parse().unwrap());
        }
    }

    // since this is five percent of the total bonded stake
    // delegating this to the validator who has the least should
    // do the trick
    let five_percent = total_bonded_stake.divide(u256!(20)).unwrap().0;
    let five_percent = Coin {
        denom: STAKING_TOKEN.clone(),
        amount: five_percent,
    };

    (has_the_least.unwrap(), five_percent)
}
