use gravity_utils::{
    clarity::{
        abi::{encode_call, Token},
        constants::ZERO_ADDRESS,
        u256, Address as EthAddress, Uint256,
    },
    types::*,
    u64_array_bigints,
    web30::{client::Web3, jsonrpc::error::Web3Error},
};

/// Gets the latest validator set nonce
pub async fn get_valset_nonce(
    contract_address: EthAddress,
    caller_address: EthAddress,
    web3: &Web3,
) -> Result<u64, Web3Error> {
    let payload = encode_call("state_lastValsetNonce()", &[]).unwrap();
    let val = web3
        .simulate_transaction(contract_address, u256!(0), payload, caller_address, None)
        .await?;
    // the go represents all nonces as u64, there's no
    // reason they should ever overflow without a user
    // submitting millions or tens of millions of dollars
    // worth of transactions. But we properly check and
    // handle that case here.
    let real_num = Uint256::from_bytes_be(&val).unwrap();
    Ok(real_num
        .try_resize_to_u64()
        .expect("Valset nonce overflow! Bridge Halt!"))
}

/// Gets the latest transaction batch nonce
pub async fn get_tx_batch_nonce(
    gravity_contract_address: EthAddress,
    erc20_contract_address: EthAddress,
    caller_address: EthAddress,
    web3: &Web3,
) -> Result<u64, Web3Error> {
    let payload = encode_call("lastBatchNonce(address)", &[erc20_contract_address.into()]).unwrap();
    let val = web3
        .simulate_transaction(
            gravity_contract_address,
            u256!(0),
            payload,
            caller_address,
            None,
        )
        .await?;
    // the go represents all nonces as u64, there's no
    // reason they should ever overflow without a user
    // submitting millions or tens of millions of dollars
    // worth of transactions. But we properly check and
    // handle that case here.
    let real_num = Uint256::from_bytes_be(&val).unwrap();
    Ok(real_num
        .try_resize_to_u64()
        .expect("TxBatch nonce overflow! Bridge Halt!"))
}

/// Gets the latest transaction batch nonce
pub async fn get_logic_call_nonce(
    gravity_contract_address: EthAddress,
    invalidation_id: Vec<u8>,
    caller_address: EthAddress,
    web3: &Web3,
) -> Result<u64, Web3Error> {
    let payload = encode_call(
        "lastLogicCallNonce(bytes32)",
        &[Token::Bytes(invalidation_id)],
    )
    .unwrap();
    let val = web3
        .simulate_transaction(
            gravity_contract_address,
            u256!(0),
            payload,
            caller_address,
            None,
        )
        .await?;
    // the go represents all nonces as u64, there's no
    // reason they should ever overflow without a user
    // submitting millions or tens of millions of dollars
    // worth of transactions. But we properly check and
    // handle that case here.
    let real_num = Uint256::from_bytes_be(&val).unwrap();
    Ok(real_num
        .try_resize_to_u64()
        .expect("LogicCall nonce overflow! Bridge Halt!"))
}

/// Gets the latest transaction batch nonce
pub async fn get_event_nonce(
    gravity_contract_address: EthAddress,
    caller_address: EthAddress,
    web3: &Web3,
) -> Result<u64, Web3Error> {
    let payload = encode_call("state_lastEventNonce()", &[]).unwrap();
    let val = web3
        .simulate_transaction(
            gravity_contract_address,
            u256!(0),
            payload,
            caller_address,
            None,
        )
        .await?;
    // the go represents all nonces as u64, there's no
    // reason they should ever overflow without a user
    // submitting millions or tens of millions of dollars
    // worth of transactions. But we properly check and
    // handle that case here.
    let real_num = Uint256::from_bytes_be(&val).unwrap();
    Ok(real_num
        .try_resize_to_u64()
        .expect("EventNonce nonce overflow! Bridge Halt!"))
}

/// Gets the gravityID
pub async fn get_gravity_id(
    contract_address: EthAddress,
    caller_address: EthAddress,
    web3: &Web3,
) -> Result<String, Web3Error> {
    let payload = encode_call("state_gravityId()", &[]).unwrap();
    let val = web3
        .simulate_transaction(contract_address, u256!(0), payload, caller_address, None)
        .await?;
    let gravity_id = String::from_utf8(val);
    match gravity_id {
        Ok(val) => Ok(val),
        Err(e) => Err(Web3Error::BadResponse(e.to_string())),
    }
}

/// Just a helper struct to represent the cost of actions on Ethereum
#[derive(Debug, Default, Clone, Copy)]
pub struct GasCost {
    /// The amount of gas spent
    pub gas: Uint256,
    /// The price of the gas
    pub gas_price: Uint256,
}

impl GasCost {
    /// Gets the total cost in Eth (or other EVM chain native token)
    /// of executing the batch
    pub fn get_total(&self) -> Uint256 {
        self.gas.checked_mul(self.gas_price).unwrap()
    }
}

/// This encodes the solidity struct ValsetArgs from the Gravity
/// contract useful for all three major contract calls
/// struct ValsetArgs {
///     address[] validators;
///     uint256[] powers;
///     uint256 valsetNonce;
///     uint256 rewardAmount;
///     address rewardToken;
// }
pub fn encode_valset_struct(valset: &Valset) -> Token {
    let (addresses, powers) = valset.to_arrays();
    let nonce = valset.nonce;
    let reward_amount = valset.reward_amount;
    // the zero address represents 'no reward' in this case we have replaced it with a 'none'
    // so that it's easy to identify if this validator set has a reward or not. Now that we're
    // going to encode it for the contract call we need return it to the magic value the contract
    // expects.
    let reward_token = valset.reward_token.unwrap_or(ZERO_ADDRESS);
    let struct_tokens = &[
        addresses.into(),
        powers.into(),
        nonce.into(),
        reward_amount.into(),
        reward_token.into(),
    ];
    Token::Struct(struct_tokens.to_vec())
}
