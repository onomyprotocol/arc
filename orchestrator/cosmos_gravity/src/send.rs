use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use ethereum_gravity::message_signatures::{
    encode_logic_call_confirm, encode_tx_batch_confirm, encode_valset_confirm,
};
use gravity_proto::{
    cosmos_sdk_proto::cosmos::base::abci::v1beta1::TxResponse,
    gravity::{
        MsgBatchSendToEthClaim, MsgCancelSendToEth, MsgConfirmBatch, MsgConfirmLogicCall,
        MsgErc20DeployedClaim, MsgLogicCallExecutedClaim, MsgRequestBatch, MsgSendToCosmosClaim,
        MsgSendToEth, MsgSetOrchestratorAddress, MsgSubmitBadSignatureEvidence, MsgValsetConfirm,
        MsgValsetUpdatedClaim,
    },
};
use gravity_utils::{
    clarity::{Address as EthAddress, PrivateKey as EthPrivateKey, Signature, Uint256},
    deep_space::{
        address::Address, coin::Coin, error::CosmosGrpcError, private_key::PrivateKey,
        utils::bytes_to_hex_str, Contact, Msg,
    },
    types::*,
};

use crate::utils::BadSignatureEvidence;

pub const MEMO: &str = "Sent using Onomy Gravity Bridge Orchestrator";
pub const TIMEOUT: Duration = Duration::from_secs(60);

/// Send a transaction updating the eth address for the sending
/// Cosmos address. The sending Cosmos address should be a validator
/// this can only be called once! Key rotation code is possible but
/// not currently implemented
pub async fn set_gravity_delegate_addresses(
    contact: &Contact,
    delegate_eth_address: EthAddress,
    delegate_cosmos_address: Address,
    private_key: PrivateKey,
    fee: Coin,
) -> Result<TxResponse, CosmosGrpcError> {
    trace!("Updating Gravity Delegate addresses");
    let our_valoper_address = private_key
        .to_address(&contact.get_prefix())
        .unwrap()
        // This works so long as the format set by the cosmos hub is maintained
        // having a main prefix followed by a series of titles for specific keys
        // this will not work if that convention is broken. This will be resolved when
        // GRPC exposes prefix endpoints (coming to upstream cosmos sdk soon)
        .to_bech32(format!("{}valoper", contact.get_prefix()))
        .unwrap();

    let msg_set_orch_address = MsgSetOrchestratorAddress {
        validator: our_valoper_address.to_string(),
        orchestrator: delegate_cosmos_address.to_string(),
        eth_address: delegate_eth_address.to_string(),
    };

    let msg = Msg::new(
        "/gravity.v1.MsgSetOrchestratorAddress",
        msg_set_orch_address,
    );
    contact
        .send_message(
            &[msg],
            Some(MEMO.to_string()),
            &[fee],
            Some(TIMEOUT),
            private_key,
        )
        .await
}

/// Send in a confirmation for an array of validator sets, it's far more efficient to send these
/// as a single message
#[allow(clippy::too_many_arguments)]
pub async fn send_valset_confirms(
    contact: &Contact,
    eth_private_key: EthPrivateKey,
    fee: Coin,
    valsets: Vec<Valset>,
    private_key: PrivateKey,
    gravity_id: String,
) -> Result<TxResponse, CosmosGrpcError> {
    let our_address = private_key.to_address(&contact.get_prefix()).unwrap();
    let our_eth_address = eth_private_key.to_address();

    let mut messages = Vec::new();

    for valset in &valsets {
        trace!("Submitting signature for valset {:?}", valset);
        let message = encode_valset_confirm(gravity_id.clone(), valset);
        let eth_signature = eth_private_key.sign_ethereum_msg(&message);
        trace!(
            "Sending valset update with address {} and sig {}",
            our_eth_address,
            bytes_to_hex_str(&eth_signature.to_bytes())
        );
        let confirm = MsgValsetConfirm {
            orchestrator: our_address.to_string(),
            eth_address: our_eth_address.to_string(),
            nonce: valset.nonce,
            signature: bytes_to_hex_str(&eth_signature.to_bytes()),
        };
        let msg = Msg::new("/gravity.v1.MsgValsetConfirm", confirm);
        messages.push(msg);
    }
    let res = contact
        .send_message(
            &messages,
            Some(MEMO.to_string()),
            &[fee],
            Some(TIMEOUT),
            private_key,
        )
        .await;
    debug!("Valset confirm res is {:?}", res);
    res
}

/// Send in a confirmation for a specific transaction batch
pub async fn send_batch_confirm(
    contact: &Contact,
    eth_private_key: EthPrivateKey,
    fee: Coin,
    transaction_batches: Vec<TransactionBatch>,
    private_key: PrivateKey,
    gravity_id: String,
) -> Result<TxResponse, CosmosGrpcError> {
    let our_address = private_key.to_address(&contact.get_prefix()).unwrap();
    let our_eth_address = eth_private_key.to_address();

    let mut messages = Vec::new();

    for batch in &transaction_batches {
        trace!("Submitting signature for batch {:?}", batch);
        let message = encode_tx_batch_confirm(gravity_id.clone(), batch);
        let eth_signature = eth_private_key.sign_ethereum_msg(&message);
        trace!(
            "Sending batch update with address {} and sig {}",
            our_eth_address,
            bytes_to_hex_str(&eth_signature.to_bytes())
        );
        let confirm = MsgConfirmBatch {
            token_contract: batch.token_contract.to_string(),
            orchestrator: our_address.to_string(),
            eth_signer: our_eth_address.to_string(),
            nonce: batch.nonce,
            signature: bytes_to_hex_str(&eth_signature.to_bytes()),
        };
        let msg = Msg::new("/gravity.v1.MsgConfirmBatch", confirm);
        messages.push(msg);
    }
    contact
        .send_message(
            &messages,
            Some(MEMO.to_string()),
            &[fee],
            Some(TIMEOUT),
            private_key,
        )
        .await
}

/// Send in a confirmation for a specific logic call
pub async fn send_logic_call_confirm(
    contact: &Contact,
    eth_private_key: EthPrivateKey,
    fee: Coin,
    logic_calls: Vec<LogicCall>,
    private_key: PrivateKey,
    gravity_id: String,
) -> Result<TxResponse, CosmosGrpcError> {
    let our_address = private_key.to_address(&contact.get_prefix()).unwrap();
    let our_eth_address = eth_private_key.to_address();

    let mut messages = Vec::new();

    for call in logic_calls {
        trace!("Submitting signature for LogicCall {:?}", call);
        let message = encode_logic_call_confirm(gravity_id.clone(), call.clone());
        let eth_signature = eth_private_key.sign_ethereum_msg(&message);
        trace!(
            "Sending LogicCall update with address {} and sig {}",
            our_eth_address,
            bytes_to_hex_str(&eth_signature.to_bytes())
        );
        let confirm = MsgConfirmLogicCall {
            orchestrator: our_address.to_string(),
            eth_signer: our_eth_address.to_string(),
            signature: bytes_to_hex_str(&eth_signature.to_bytes()),
            invalidation_id: bytes_to_hex_str(&call.invalidation_id),
            invalidation_nonce: call.invalidation_nonce,
        };
        let msg = Msg::new("/gravity.v1.MsgConfirmLogicCall", confirm);
        messages.push(msg);
    }
    contact
        .send_message(
            &messages,
            Some(MEMO.to_string()),
            &[fee],
            Some(TIMEOUT),
            private_key,
        )
        .await
}

#[allow(clippy::too_many_arguments)]
pub async fn send_ethereum_claims(
    contact: &Contact,
    private_key: PrivateKey,
    deposits: Vec<SendToCosmosEvent>,
    withdraws: Vec<TransactionBatchExecutedEvent>,
    erc20_deploys: Vec<Erc20DeployedEvent>,
    logic_calls: Vec<LogicCallExecutedEvent>,
    valsets: Vec<ValsetUpdatedEvent>,
    fee: Coin,
) -> Result<TxResponse, CosmosGrpcError> {
    let our_address = private_key.to_address(&contact.get_prefix()).unwrap();

    // This sorts oracle messages by event nonce before submitting them. It's not a pretty implementation because
    // we're missing an intermediary layer of abstraction. We could implement 'EventTrait' and then implement sort
    // for it, but then when we go to transform 'EventTrait' objects into GravityMsg enum values we'll have all sorts
    // of issues extracting the inner object from the TraitObject. Likewise we could implement sort of GravityMsg but that
    // would require a truly horrendous (nearly 100 line) match statement to deal with all combinations. That match statement
    // could be reduced by adding two traits to sort against but really this is the easiest option.
    //
    // We index the events by event nonce in a sorted hashmap, skipping the need to sort it later
    let mut ordered_msgs = BTreeMap::new();
    for deposit in deposits {
        let claim = MsgSendToCosmosClaim {
            event_nonce: deposit.event_nonce,
            block_height: deposit.block_height.try_resize_to_u64().unwrap(),
            token_contract: deposit.erc20.to_string(),
            amount: deposit.amount.to_string(),
            cosmos_receiver: deposit.destination,
            ethereum_sender: deposit.sender.to_string(),
            orchestrator: our_address.to_string(),
        };
        let msg = Msg::new("/gravity.v1.MsgSendToCosmosClaim", claim);
        ordered_msgs.insert(deposit.event_nonce, msg);
    }
    for withdraw in withdraws {
        let claim = MsgBatchSendToEthClaim {
            event_nonce: withdraw.event_nonce,
            block_height: withdraw.block_height.try_resize_to_u64().unwrap(),
            token_contract: withdraw.erc20.to_string(),
            batch_nonce: withdraw.batch_nonce,
            orchestrator: our_address.to_string(),
            reward_recipient: withdraw.reward_recipient.to_string(),
        };
        let msg = Msg::new("/gravity.v1.MsgBatchSendToEthClaim", claim);
        ordered_msgs.insert(withdraw.event_nonce, msg);
    }
    for deploy in erc20_deploys {
        let claim = MsgErc20DeployedClaim {
            event_nonce: deploy.event_nonce,
            block_height: deploy.block_height.try_resize_to_u64().unwrap(),
            cosmos_denom: deploy.cosmos_denom,
            token_contract: deploy.erc20_address.to_string(),
            name: deploy.name,
            symbol: deploy.symbol,
            decimals: deploy.decimals as u64,
            orchestrator: our_address.to_string(),
        };
        let msg = Msg::new("/gravity.v1.MsgERC20DeployedClaim", claim);
        ordered_msgs.insert(deploy.event_nonce, msg);
    }
    for call in logic_calls {
        let claim = MsgLogicCallExecutedClaim {
            event_nonce: call.event_nonce,
            block_height: call.block_height.try_resize_to_u64().unwrap(),
            invalidation_id: call.invalidation_id,
            invalidation_nonce: call.invalidation_nonce,
            orchestrator: our_address.to_string(),
        };
        let msg = Msg::new("/gravity.v1.MsgLogicCallExecutedClaim", claim);
        ordered_msgs.insert(call.event_nonce, msg);
    }
    for valset in valsets {
        let claim = MsgValsetUpdatedClaim {
            event_nonce: valset.event_nonce,
            valset_nonce: valset.valset_nonce,
            block_height: valset.block_height.try_resize_to_u64().unwrap(),
            members: valset.members.iter().map(|v| v.into()).collect(),
            reward_amount: valset.reward_amount.to_string(),
            reward_denom: valset.reward_denom,
            reward_recipient: valset.reward_recipient,
            orchestrator: our_address.to_string(),
        };
        let msg = Msg::new("/gravity.v1.MsgValsetUpdatedClaim", claim);
        ordered_msgs.insert(valset.event_nonce, msg);
    }

    let msgs: Vec<Msg> = ordered_msgs.into_iter().map(|(_, v)| v).collect();

    contact
        .send_message(&msgs, None, &[fee], Some(TIMEOUT), private_key)
        .await
}

/// Sends tokens from Cosmos to Ethereum. These tokens will not be sent immediately instead
/// they will require some time to be included in a batch. Note that there are two fees
/// one is the fee to be sent to Ethereum, which must be the same denom as the amount
/// the other is the Cosmos chain fee, which can be any allowed coin
pub async fn send_to_eth(
    private_key: PrivateKey,
    destination: EthAddress,
    amount: Coin,
    bridge_fee: Coin,
    tx_fee: Coin,
    contact: &Contact,
) -> Result<TxResponse, CosmosGrpcError> {
    let our_address = private_key.to_address(&contact.get_prefix()).unwrap();

    let coins = &vec![amount.clone(), bridge_fee.clone(), tx_fee.clone()];
    if let Err(e) = validate_balance_sufficiency(contact, our_address, coins.to_vec()).await {
        return Err(e);
    }

    let msg_send_to_eth = MsgSendToEth {
        sender: our_address.to_string(),
        eth_dest: destination.to_string(),
        amount: Some(amount.into()),
        bridge_fee: Some(bridge_fee.clone().into()),
    };

    let msg = Msg::new("/gravity.v1.MsgSendToEth", msg_send_to_eth);
    contact
        .send_message(
            &[msg],
            Some(MEMO.to_string()),
            &[tx_fee],
            Some(TIMEOUT),
            private_key,
        )
        .await
}

async fn validate_balance_sufficiency(
    contact: &Contact,
    user_address: Address,
    coins: Vec<Coin>,
) -> Result<(), CosmosGrpcError> {
    let mut balances_map: HashMap<String, Uint256> = HashMap::new();
    let balances = contact.get_balances(user_address).await.unwrap();
    for balance in balances {
        balances_map.insert(balance.denom, balance.amount);
    }
    for coin in coins {
        let denom = coin.clone().denom;
        let denom_amount = balances_map.get(&denom);
        if denom_amount.is_none() {
            return Err(CosmosGrpcError::BadInput(format!(
                "No balance of {} denom",
                denom,
            )));
        }

        if *denom_amount.unwrap() < coin.amount {
            return Err(CosmosGrpcError::BadInput(format!(
                "Insufficient balance {} denom",
                denom,
            )));
        } else {
        }

        let sub_amount = denom_amount.unwrap().checked_sub(coin.amount).unwrap();
        balances_map.insert(denom, sub_amount);
    }

    Ok(())
}

pub async fn send_request_batch(
    private_key: PrivateKey,
    denom: String,
    fee: Option<Coin>,
    contact: &Contact,
) -> Result<TxResponse, CosmosGrpcError> {
    let our_address = private_key.to_address(&contact.get_prefix()).unwrap();

    let msg_request_batch = MsgRequestBatch {
        sender: our_address.to_string(),
        denom,
    };
    let msg = Msg::new("/gravity.v1.MsgRequestBatch", msg_request_batch);

    let fee: Vec<Coin> = match fee {
        Some(fee) => vec![fee],
        None => vec![],
    };
    contact
        .send_message(
            &[msg],
            Some(MEMO.to_string()),
            &fee,
            Some(TIMEOUT),
            private_key,
        )
        .await
}

/// Sends evidence of a bad signature to the chain to slash the malicious validator
/// who signed an invalid message with their Ethereum key
pub async fn submit_bad_signature_evidence(
    private_key: PrivateKey,
    fee: Coin,
    contact: &Contact,
    signed_object: BadSignatureEvidence,
    signature: Signature,
) -> Result<TxResponse, CosmosGrpcError> {
    let our_address = private_key.to_address(&contact.get_prefix()).unwrap();

    let any = signed_object.to_any();

    let msg_submit_bad_signature_evidence = MsgSubmitBadSignatureEvidence {
        subject: Some(any),
        signature: bytes_to_hex_str(&signature.to_bytes()),
        sender: our_address.to_string(),
    };

    let msg = Msg::new(
        "/gravity.v1.MsgSubmitBadSignatureEvidence",
        msg_submit_bad_signature_evidence,
    );
    contact
        .send_message(
            &[msg],
            Some(MEMO.to_string()),
            &[fee],
            Some(TIMEOUT),
            private_key,
        )
        .await
}

/// Cancels a user provided SendToEth transaction, provided it's not already in a batch
/// you should check with `QueryPendingSendToEth`
pub async fn cancel_send_to_eth(
    private_key: PrivateKey,
    fee: Coin,
    contact: &Contact,
    transaction_id: u64,
) -> Result<TxResponse, CosmosGrpcError> {
    let our_address = private_key.to_address(&contact.get_prefix()).unwrap();

    let msg_cancel_send_to_eth = MsgCancelSendToEth {
        transaction_id,
        sender: our_address.to_string(),
    };

    let msg = Msg::new("/gravity.v1.MsgCancelSendToEth", msg_cancel_send_to_eth);
    contact
        .send_message(
            &[msg],
            Some(MEMO.to_string()),
            &[fee],
            Some(TIMEOUT),
            private_key,
        )
        .await
}
