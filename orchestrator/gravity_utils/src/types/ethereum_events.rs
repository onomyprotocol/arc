//! This file parses the Gravity contract ethereum events. Note that there is no Ethereum ABI unpacking implementation. Instead each event
//! is parsed directly from it's binary representation. This is technical debt within this implementation. It's quite easy to parse any
//! individual event manually but a generic decoder can be quite challenging to implement. A proper implementation would probably closely
//! mirror Serde and perhaps even become a serde crate for Ethereum ABI decoding
//! For now reference the ABI encoding document here https://docs.soliditylang.org/en/v0.8.3/abi-spec.html

// TODO this file needs static assertions that prevent it from compiling on 16 bit systems.
// we assume a system bit width of at least 32

use std::unimplemented;

use clarity::{Address as EthAddress, Uint256};
use deep_space::{utils::bytes_to_hex_str, Address as CosmosAddress};
use serde::{Deserialize, Serialize};
use web30::types::Log;

use super::ValsetMember;
use crate::error::GravityError;

/// Used to limit the length of variable length user provided inputs like
/// ERC20 names and deposit destination strings
const ONE_MEGABYTE: usize = 1000usize.pow(3);
const U32_MAX: Uint256 = Uint256::from_u32(u32::MAX);
const U64_MAX: Uint256 = Uint256::from_u64(u64::MAX);

/// A parsed struct representing the Ethereum event fired by the Gravity contract
/// when the validator set is updated.
#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ValsetUpdatedEvent {
    pub valset_nonce: u64,
    pub event_nonce: u64,
    pub block_height: Uint256,
    pub reward_amount: Uint256,
    pub reward_denom: String,
    pub reward_recipient: String,
    pub members: Vec<ValsetMember>,
}

/// special return struct just for the data bytes components
#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, PartialEq, Hash)]
struct ValsetDataBytes {
    pub event_nonce: u64,
    pub reward_amount: Uint256,
    pub reward_denom: String,
    pub reward_recipient: String,
    pub members: Vec<ValsetMember>,
}

// TODO refactor the entire file to parse by arg index instead of code duplication.
impl ValsetUpdatedEvent {
    /// Decodes the data bytes of a valset log event, separated for easy testing
    fn decode_data_bytes(input: &[u8]) -> Result<ValsetDataBytes, GravityError> {
        if input.len() < 7 * 32 {
            return Err(GravityError::ValidationError(
                "too short for ValsetUpdatedEventData".to_string(),
            ));
        }

        // event nonce
        let index_start = 0;
        let index_end = index_start + 32;
        let nonce_data = &input[index_start..index_end];
        let event_nonce = Uint256::from_bytes_be(nonce_data).unwrap();
        if event_nonce > U64_MAX {
            return Err(GravityError::ValidationError(
                "Nonce overflow, probably incorrect parsing".into(),
            ));
        }
        let event_nonce: u64 = event_nonce.to_string().parse().unwrap();

        // reward amount
        let index_start = 32;
        let index_end = index_start + 32;
        let reward_amount_data = &input[index_start..index_end];
        let reward_amount = Uint256::from_bytes_be(reward_amount_data).unwrap();

        let reward_denom = match parse_string(input, 2) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        let reward_recipient = match parse_string(input, 3) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        let validators_addresses: Vec<EthAddress> = match parse_address_array(input, 4) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        let validators_powers: Vec<Uint256> = match parse_uint256_array(input, 5) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        if validators_addresses.len() != validators_powers.len() {
            return Err(GravityError::ValidationError(
                "validators_addresses len != validators_powers len".to_string(),
            ));
        }

        let mut validators = Vec::new();
        for i in 0..validators_powers.len() {
            let power = validators_powers[i];
            if power > U64_MAX {
                return Err(GravityError::ValidationError(
                    "Power greater than u64::MAX, probably incorrect parsing".to_string(),
                ));
            }
            let power: u64 = power.to_string().parse().unwrap();
            let eth_address = validators_addresses[i];
            validators.push(ValsetMember { power, eth_address })
        }

        Ok(ValsetDataBytes {
            event_nonce,
            members: validators,
            reward_amount,
            reward_denom,
            reward_recipient,
        })
    }

    /// This function is not an abi compatible bytes parser, but it's actually
    /// not hard at all to extract data like this by hand.
    pub fn from_log(input: &Log) -> Result<ValsetUpdatedEvent, GravityError> {
        // we have one indexed event so we should find two indexes, one the event itself
        // and one the indexed nonce
        if input.topics.get(1).is_none() {
            return Err(GravityError::ValidationError("Too few topics".to_string()));
        }
        let valset_nonce_data = &input.topics[1];
        let valset_nonce = Uint256::from_bytes_be(valset_nonce_data).unwrap();
        if valset_nonce > U64_MAX {
            return Err(GravityError::ValidationError(
                "Nonce overflow, probably incorrect parsing".to_string(),
            ));
        }
        let valset_nonce: u64 = valset_nonce.to_string().parse().unwrap();

        let block_height = if let Some(bn) = input.block_number {
            if bn > U64_MAX {
                return Err(GravityError::ValidationError(
                    "Event nonce overflow! probably incorrect parsing".to_string(),
                ));
            } else {
                bn
            }
        } else {
            return Err(GravityError::ValidationError(
                "Log does not have block number, we only search logs already in blocks?"
                    .to_string(),
            ));
        };

        let decoded_bytes = Self::decode_data_bytes(&input.data)?;

        Ok(ValsetUpdatedEvent {
            valset_nonce,
            event_nonce: decoded_bytes.event_nonce,
            block_height,
            reward_amount: decoded_bytes.reward_amount,
            reward_denom: decoded_bytes.reward_denom,
            reward_recipient: decoded_bytes.reward_recipient,
            members: decoded_bytes.members,
        })
    }

    pub fn from_logs(input: &[Log]) -> Result<Vec<ValsetUpdatedEvent>, GravityError> {
        let mut res = Vec::new();
        for item in input {
            res.push(ValsetUpdatedEvent::from_log(item)?);
        }
        Ok(res)
    }
    /// returns all values in the array with event nonces greater
    /// than the provided value
    pub fn filter_by_event_nonce(event_nonce: u64, input: &[Self]) -> Vec<Self> {
        let mut ret = Vec::new();
        for item in input {
            if item.event_nonce > event_nonce {
                ret.push(item.clone())
            }
        }
        ret
    }
}

/// A parsed struct representing the Ethereum event fired by the Gravity contract when
/// a transaction batch is executed.
#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct TransactionBatchExecutedEvent {
    /// the nonce attached to the transaction batch that follows
    /// it throughout it's lifecycle
    pub batch_nonce: u64,
    /// The block height this event occurred at
    pub block_height: Uint256,
    /// The ERC20 token contract address for the batch executed, since batches are uniform
    /// in token type there is only one
    pub erc20: EthAddress,
    /// the event nonce representing a unique ordering of events coming out
    /// of the Gravity solidity contract. Ensuring that these events can only be played
    /// back in order
    pub event_nonce: u64,
}

impl TransactionBatchExecutedEvent {
    pub fn from_log(input: &Log) -> Result<TransactionBatchExecutedEvent, GravityError> {
        if let (Some(batch_nonce_data), Some(erc20_data)) =
            (input.topics.get(1), input.topics.get(2))
        {
            let batch_nonce = Uint256::from_bytes_be(batch_nonce_data).unwrap();
            let erc20 = EthAddress::from_slice(&erc20_data[12..32])?;
            let event_nonce = Uint256::from_bytes_be(&input.data).unwrap();
            let block_height = if let Some(bn) = input.block_number {
                if bn > U64_MAX {
                    return Err(GravityError::ValidationError(
                        "Block height overflow! probably incorrect parsing".to_string(),
                    ));
                } else {
                    bn
                }
            } else {
                return Err(GravityError::ValidationError(
                    "Log does not have block number, we only search logs already in blocks?"
                        .to_string(),
                ));
            };
            if event_nonce > U64_MAX || batch_nonce > U64_MAX || block_height > U64_MAX {
                Err(GravityError::ValidationError(
                    "Event nonce overflow, probably incorrect parsing".to_string(),
                ))
            } else {
                let batch_nonce: u64 = batch_nonce.to_string().parse().unwrap();
                let event_nonce: u64 = event_nonce.to_string().parse().unwrap();
                Ok(TransactionBatchExecutedEvent {
                    batch_nonce,
                    block_height,
                    erc20,
                    event_nonce,
                })
            }
        } else {
            Err(GravityError::ValidationError("Too few topics".to_string()))
        }
    }
    pub fn from_logs(input: &[Log]) -> Result<Vec<TransactionBatchExecutedEvent>, GravityError> {
        let mut res = Vec::new();
        for item in input {
            res.push(TransactionBatchExecutedEvent::from_log(item)?);
        }
        Ok(res)
    }
    /// returns all values in the array with event nonces greater
    /// than the provided value
    pub fn filter_by_event_nonce(event_nonce: u64, input: &[Self]) -> Vec<Self> {
        let mut ret = Vec::new();
        for item in input {
            if item.event_nonce > event_nonce {
                ret.push(item.clone())
            }
        }
        ret
    }
}

/// A parsed struct representing the Ethereum event fired when someone makes a deposit
/// on the Gravity contract
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct SendToCosmosEvent {
    /// The token contract address for the deposit
    pub erc20: EthAddress,
    /// The Ethereum Sender
    pub sender: EthAddress,
    /// The Cosmos destination, this is a raw value from the Ethereum contract
    /// and therefore could be provided by an attacker. If the string is valid
    /// utf-8 it will be included here, if it is invalid utf8 we will provide
    /// an empty string. Values over 1mb of text are not permitted and will also
    /// be presented as empty
    pub destination: String,
    /// the validated destination is the destination string parsed and interpreted
    /// as a valid Bech32 Cosmos address, if this is not possible the value is none
    pub validated_destination: Option<CosmosAddress>,
    /// The amount of the erc20 token that is being sent
    pub amount: Uint256,
    /// The transaction's nonce, used to make sure there can be no accidental duplication
    pub event_nonce: u64,
    /// The block height this event occurred at
    pub block_height: Uint256,
}

/// struct for holding the data encoded fields
/// of a send to Cosmos event for unit testing
#[derive(Eq, PartialEq, Debug)]
struct SendToCosmosEventData {
    /// The Cosmos destination, None for an invalid deposit address
    pub destination: String,
    /// The amount of the erc20 token that is being sent
    pub amount: Uint256,
    /// The transaction's nonce, used to make sure there can be no accidental duplication
    pub event_nonce: Uint256,
}

impl SendToCosmosEvent {
    pub fn from_log(input: &Log) -> Result<SendToCosmosEvent, GravityError> {
        let topics = (input.topics.get(1), input.topics.get(2));
        if let (Some(erc20_data), Some(sender_data)) = topics {
            let erc20 = EthAddress::from_slice(&erc20_data[12..32])?;
            let sender = EthAddress::from_slice(&sender_data[12..32])?;
            let block_height = if let Some(bn) = input.block_number {
                if bn > U64_MAX {
                    return Err(GravityError::ValidationError(
                        "Block height overflow! probably incorrect parsing".to_string(),
                    ));
                } else {
                    bn
                }
            } else {
                return Err(GravityError::ValidationError(
                    "Log does not have block number, we only search logs already in blocks?"
                        .to_string(),
                ));
            };

            let data = SendToCosmosEvent::decode_data_bytes(&input.data)?;
            if data.event_nonce > U64_MAX || block_height > U64_MAX {
                Err(GravityError::ValidationError(
                    "Event nonce overflow, probably incorrect parsing".to_string(),
                ))
            } else {
                let event_nonce: u64 = data.event_nonce.to_string().parse().unwrap();
                let validated_destination = match data.destination.parse() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        if data.destination.len() < 1000 {
                            warn!("Event nonce {} sends tokens to {} which is invalid bech32, these funds will be allocated to the community pool", event_nonce, data.destination);
                        } else {
                            warn!("Event nonce {} sends tokens to a destination which is invalid bech32, these funds will be allocated to the community pool", event_nonce);
                        }
                        None
                    }
                };
                Ok(SendToCosmosEvent {
                    erc20,
                    sender,
                    destination: data.destination,
                    validated_destination,
                    amount: data.amount,
                    event_nonce,
                    block_height,
                })
            }
        } else {
            Err(GravityError::ValidationError("Too few topics".to_string()))
        }
    }
    fn decode_data_bytes(data: &[u8]) -> Result<SendToCosmosEventData, GravityError> {
        if data.len() < 4 * 32 {
            return Err(GravityError::ValidationError(
                "too short for SendToCosmosEventData".to_string(),
            ));
        }

        let amount = Uint256::from_bytes_be(&data[32..64]).unwrap();
        let event_nonce = Uint256::from_bytes_be(&data[64..96]).unwrap();

        // discard words three and four which contain the data type and length
        let destination_str_len_start = 3 * 32;
        let destination_str_len_end = 4 * 32;
        let destination_str_len =
            Uint256::from_bytes_be(&data[destination_str_len_start..destination_str_len_end])
                .unwrap();

        if destination_str_len > U32_MAX {
            return Err(GravityError::ValidationError(
                "denom length overflow, probably incorrect parsing".to_string(),
            ));
        }
        let destination_str_len: usize = match destination_str_len.try_resize_to_usize() {
            Some(v) => v,
            None => {
                return Err(GravityError::ValidationError(
                    "Can't resize to usize".into(),
                ))
            }
        };

        let destination_str_start = 4 * 32;
        let destination_str_end = destination_str_start + destination_str_len;

        if data.len() < destination_str_end {
            return Err(GravityError::ValidationError(
                "Incorrect length for dynamic data".to_string(),
            ));
        }

        let destination = &data[destination_str_start..destination_str_end];

        let dest = String::from_utf8(destination.to_vec());
        if dest.is_err() {
            if destination.len() < 1000 {
                warn!("Event nonce {} sends tokens to {} which is invalid utf-8, these funds will be allocated to the community pool", event_nonce, bytes_to_hex_str(destination));
            } else {
                warn!("Event nonce {} sends tokens to a destination that is invalid utf-8, these funds will be allocated to the community pool", event_nonce);
            }
            return Ok(SendToCosmosEventData {
                destination: String::new(),
                event_nonce,
                amount,
            });
        }
        // whitespace can not be a valid part of a bech32 address, so we can safely trim it
        let dest = dest.unwrap().trim().to_string();

        if dest.as_bytes().len() > ONE_MEGABYTE {
            warn!("Event nonce {} sends tokens to a destination that exceeds the length limit, these funds will be allocated to the community pool", event_nonce);
            Ok(SendToCosmosEventData {
                destination: String::new(),
                event_nonce,
                amount,
            })
        } else {
            Ok(SendToCosmosEventData {
                destination: dest,
                event_nonce,
                amount,
            })
        }
    }
    pub fn from_logs(input: &[Log]) -> Result<Vec<SendToCosmosEvent>, GravityError> {
        let mut res = Vec::new();
        for item in input {
            res.push(SendToCosmosEvent::from_log(item)?);
        }
        Ok(res)
    }
    /// returns all values in the array with event nonces greater
    /// than the provided value
    pub fn filter_by_event_nonce(event_nonce: u64, input: &[Self]) -> Vec<Self> {
        let mut ret = Vec::new();
        for item in input {
            if item.event_nonce > event_nonce {
                ret.push(item.clone())
            }
        }
        ret
    }
}

/// A parsed struct representing the Ethereum event fired when someone uses the Gravity
/// contract to deploy a new ERC20 contract representing a Cosmos asset
#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Erc20DeployedEvent {
    /// The denom on the Cosmos chain this contract is intended to represent
    pub cosmos_denom: String,
    /// The ERC20 address of the deployed contract, this may or may not be adopted
    /// by the Cosmos chain as the contract for this asset
    pub erc20_address: EthAddress,
    /// The name of the token in the ERC20 contract, should match the Cosmos denom
    /// but it is up to the Cosmos module to check that
    pub name: String,
    /// The symbol for the token in the ERC20 contract
    pub symbol: String,
    /// The number of decimals required to represent the smallest unit of this token
    pub decimals: u8,
    pub event_nonce: u64,
    pub block_height: Uint256,
}

/// struct for holding the data encoded fields
/// of a Erc20DeployedEvent for unit testing
#[derive(Eq, PartialEq, Debug)]
struct Erc20DeployedEventData {
    /// The denom on the Cosmos chain this contract is intended to represent
    pub cosmos_denom: String,
    /// The name of the token in the ERC20 contract, should match the Cosmos denom
    /// but it is up to the Cosmos module to check that
    pub name: String,
    /// The symbol for the token in the ERC20 contract
    pub symbol: String,
    /// The number of decimals required to represent the smallest unit of this token
    pub decimals: u8,
    pub event_nonce: u64,
}

impl Erc20DeployedEvent {
    pub fn from_log(input: &Log) -> Result<Erc20DeployedEvent, GravityError> {
        let token_contract = input.topics.get(1);
        if let Some(new_token_contract_data) = token_contract {
            let erc20 = EthAddress::from_slice(&new_token_contract_data[12..32])?;

            let block_height = if let Some(bn) = input.block_number {
                if bn > U64_MAX {
                    return Err(GravityError::ValidationError(
                        "Event nonce overflow! probably incorrect parsing".to_string(),
                    ));
                } else {
                    bn
                }
            } else {
                return Err(GravityError::ValidationError(
                    "Log does not have block number, we only search logs already in blocks?"
                        .to_string(),
                ));
            };

            let data = Erc20DeployedEvent::decode_data_bytes(&input.data)?;

            Ok(Erc20DeployedEvent {
                cosmos_denom: data.cosmos_denom,
                name: data.name,
                decimals: data.decimals,
                event_nonce: data.event_nonce,
                erc20_address: erc20,
                symbol: data.symbol,
                block_height,
            })
        } else {
            Err(GravityError::ValidationError("Too few topics".to_string()))
        }
    }
    fn decode_data_bytes(data: &[u8]) -> Result<Erc20DeployedEventData, GravityError> {
        if data.len() < 6 * 32 {
            return Err(GravityError::ValidationError(
                "too short for Erc20DeployedEventData".to_string(),
            ));
        }

        // discard index 2 as it only contains type data
        let index_start = 3 * 32;
        let index_end = index_start + 32;

        let decimals = Uint256::from_bytes_be(&data[index_start..index_end]).unwrap();
        if decimals > Uint256::from_u8(u8::MAX) {
            return Err(GravityError::ValidationError(
                "Decimals overflow, probably incorrect parsing".to_string(),
            ));
        }
        let decimals: u8 = decimals.to_string().parse().unwrap();

        let index_start = 4 * 32;
        let index_end = index_start + 32;
        let nonce = Uint256::from_bytes_be(&data[index_start..index_end]).unwrap();
        if nonce > U64_MAX {
            return Err(GravityError::ValidationError(
                "Nonce overflow, probably incorrect parsing".to_string(),
            ));
        }
        let event_nonce: u64 = nonce.to_string().parse().unwrap();

        let index_start = 5 * 32;
        let index_end = index_start + 32;
        let denom_len = Uint256::from_bytes_be(&data[index_start..index_end]).unwrap();
        // it's not probable that we have 4+ gigabytes of event data
        if denom_len > U32_MAX {
            return Err(GravityError::ValidationError(
                "denom length overflow, probably incorrect parsing".to_string(),
            ));
        }
        let denom_len: usize = match denom_len.try_resize_to_usize() {
            Some(v) => v,
            None => {
                return Err(GravityError::ValidationError(
                    "Can't resize to usize".into(),
                ))
            }
        };

        let index_start = 6 * 32;
        let index_end = index_start + denom_len;
        let denom = String::from_utf8(data[index_start..index_end].to_vec());
        trace!("Denom {:?}", denom);
        if denom.is_err() {
            warn!("Deployed ERC20 has invalid utf8, will not be adopted");
            // we must return a dummy event in order to finish processing
            // otherwise we halt the oracle
            return Ok(Erc20DeployedEventData {
                cosmos_denom: String::new(),
                name: String::new(),
                symbol: String::new(),
                decimals: 0,
                event_nonce,
            });
        }
        let denom = denom.unwrap();
        if denom.len() > ONE_MEGABYTE {
            warn!("Deployed ERC20 is too large! will not be adopted");
            // we must return a dummy event in order to finish processing
            // otherwise we halt the oracle
            return Ok(Erc20DeployedEventData {
                cosmos_denom: String::new(),
                name: String::new(),
                symbol: String::new(),
                decimals: 0,
                event_nonce,
            });
        }

        // beyond this point we are parsing strings placed
        // after a variable length string and we will need to compute offsets

        // this trick computes the next 32 byte (256 bit) word index, then multiplies by
        // 32 to get the bytes offset, this is required since we have dynamic length types but
        // the next entry always starts on a round 32 byte word.
        let index_start = ((index_end + 31) / 32) * 32;
        let index_end = index_start + 32;

        if data.len() < index_end {
            return Err(GravityError::ValidationError(
                "Erc20DeployedEvent dynamic data too short".to_string(),
            ));
        }

        let erc20_name_len = Uint256::from_bytes_be(&data[index_start..index_end]).unwrap();
        // it's not probable that we have 4+ gigabytes of event data
        if erc20_name_len > U32_MAX {
            return Err(GravityError::ValidationError(
                "ERC20 Name length overflow, probably incorrect parsing".to_string(),
            ));
        }
        let erc20_name_len: usize = match erc20_name_len.try_resize_to_usize() {
            Some(v) => v,
            None => {
                return Err(GravityError::ValidationError(
                    "Can't resize to usize".into(),
                ))
            }
        };

        let index_start = index_end;
        let index_end = index_start + erc20_name_len;

        if data.len() < index_end {
            return Err(GravityError::ValidationError(
                "Erc20DeployedEvent dynamic data too short".to_string(),
            ));
        }

        let erc20_name = String::from_utf8(data[index_start..index_end].to_vec());
        if erc20_name.is_err() {
            warn!("Deployed ERC20 has invalid utf8, will not be adopted");
            // we must return a dummy event in order to finish processing
            // otherwise we halt the oracle
            return Ok(Erc20DeployedEventData {
                cosmos_denom: String::new(),
                name: String::new(),
                symbol: String::new(),
                decimals: 0,
                event_nonce,
            });
        }
        trace!("ERC20 Name {:?}", erc20_name);
        let erc20_name = erc20_name.unwrap();
        if erc20_name.len() > ONE_MEGABYTE {
            warn!("Deployed ERC20 is too large! will not be adopted");
            // we must return a dummy event in order to finish processing
            // otherwise we halt the oracle
            return Ok(Erc20DeployedEventData {
                cosmos_denom: String::new(),
                name: String::new(),
                symbol: String::new(),
                decimals: 0,
                event_nonce,
            });
        }

        let index_start = ((index_end + 31) / 32) * 32;
        let index_end = index_start + 32;

        if data.len() < index_end {
            return Err(GravityError::ValidationError(
                "Erc20DeployedEvent dynamic data too short".to_string(),
            ));
        }

        let symbol_len = Uint256::from_bytes_be(&data[index_start..index_end]).unwrap();
        // it's not probable that we have 4+ gigabytes of event data
        if symbol_len > U32_MAX {
            return Err(GravityError::ValidationError(
                "Symbol length overflow, probably incorrect parsing".to_string(),
            ));
        }
        let symbol_len: usize = match symbol_len.try_resize_to_usize() {
            Some(v) => v,
            None => {
                return Err(GravityError::ValidationError(
                    "Can't resize to usize".into(),
                ))
            }
        };

        let index_start = index_end;
        let index_end = index_start + symbol_len;

        if data.len() < index_end {
            return Err(GravityError::ValidationError(
                "Erc20DeployedEvent dynamic data too short".to_string(),
            ));
        }

        let symbol = String::from_utf8(data[index_start..index_end].to_vec());
        trace!("Symbol {:?}", symbol);
        if symbol.is_err() {
            warn!("Deployed ERC20 has invalid utf8, will not be adopted");
            // we must return a dummy event in order to finish processing
            // otherwise we halt the oracle
            return Ok(Erc20DeployedEventData {
                cosmos_denom: String::new(),
                name: String::new(),
                symbol: String::new(),
                decimals: 0,
                event_nonce,
            });
        }
        let symbol = symbol.unwrap();
        if symbol.len() > ONE_MEGABYTE {
            warn!("Deployed ERC20 is too large! will not be adopted");
            // we must return a dummy event in order to finish processing
            // otherwise we halt the oracle
            return Ok(Erc20DeployedEventData {
                cosmos_denom: String::new(),
                name: String::new(),
                symbol: String::new(),
                decimals: 0,
                event_nonce,
            });
        }

        Ok(Erc20DeployedEventData {
            cosmos_denom: denom,
            name: erc20_name,
            symbol,
            decimals,
            event_nonce,
        })
    }
    pub fn from_logs(input: &[Log]) -> Result<Vec<Erc20DeployedEvent>, GravityError> {
        let mut res = Vec::new();
        for item in input {
            res.push(Erc20DeployedEvent::from_log(item)?);
        }
        Ok(res)
    }
    /// returns all values in the array with event nonces greater
    /// than the provided value
    pub fn filter_by_event_nonce(event_nonce: u64, input: &[Self]) -> Vec<Self> {
        let mut ret = Vec::new();
        for item in input {
            if item.event_nonce > event_nonce {
                ret.push(item.clone())
            }
        }
        ret
    }
}

/// A parsed struct representing the Ethereum event fired when someone uses the Gravity
/// contract to deploy a new ERC20 contract representing a Cosmos asset
#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct LogicCallExecutedEvent {
    pub invalidation_id: Vec<u8>,
    pub invalidation_nonce: u64,
    pub return_data: Vec<u8>,
    pub event_nonce: u64,
    pub block_height: Uint256,
}

impl LogicCallExecutedEvent {
    pub fn from_log(_input: &Log) -> Result<LogicCallExecutedEvent, GravityError> {
        unimplemented!()
    }
    pub fn from_logs(input: &[Log]) -> Result<Vec<LogicCallExecutedEvent>, GravityError> {
        let mut res = Vec::new();
        for item in input {
            res.push(LogicCallExecutedEvent::from_log(item)?);
        }
        Ok(res)
    }
    /// returns all values in the array with event nonces greater
    /// than the provided value
    pub fn filter_by_event_nonce(event_nonce: u64, input: &[Self]) -> Vec<Self> {
        let mut ret = Vec::new();
        for item in input {
            if item.event_nonce > event_nonce {
                ret.push(item.clone())
            }
        }
        ret
    }
}

fn parse_string(data: &[u8], arg_index: usize) -> Result<String, GravityError> {
    // fetching the string from the first 32 bytes
    let offset_start = arg_index * 32;
    let offset_end = offset_start + 32;
    let offset = Uint256::from_bytes_be(&data[offset_start..offset_end]).unwrap();

    // parse start and end of the string length
    let len_start_index = match offset.try_resize_to_usize() {
        Some(v) => v,
        None => {
            return Err(GravityError::ValidationError(
                "Can't resize to usize".into(),
            ))
        }
    };
    let len_end_index = len_start_index + 32;

    let len = Uint256::from_bytes_be(&data[len_start_index..len_end_index]).unwrap();
    let len: usize = match len.try_resize_to_usize() {
        Some(v) => v,
        None => {
            return Err(GravityError::ValidationError(
                "Can't resize to usize".into(),
            ))
        }
    };

    if len == 0 {
        return Ok("".to_string());
    }

    let start_index = len_end_index;
    // based on string length compute how many next bytes will be taken for that string
    let end_index = start_index + (((len - 1) / 32) + 1) * 32;

    match String::from_utf8(data[start_index..end_index].to_vec()) {
        Ok(s) => Ok(s.trim_matches(char::from(0)).to_string()),
        Err(e) => Err(GravityError::ValidationError(format!(
            "Can't convert bytes from {:?} to {:?} to string, err: {:?}",
            start_index, end_index, e
        ))),
    }
}

fn parse_address_array(data: &[u8], arg_index: usize) -> Result<Vec<EthAddress>, GravityError> {
    // fetching the string from the first 32 bytes
    let offset_start = arg_index * 32;
    let offset_end = offset_start + 32;
    let offset = Uint256::from_bytes_be(&data[offset_start..offset_end]).unwrap();

    // parse start and end of the string length
    let len_start_index: usize = match offset.try_resize_to_usize() {
        Some(v) => v,
        None => {
            return Err(GravityError::ValidationError(
                "Can't resize to usize".into(),
            ))
        }
    };

    let len_end_index = len_start_index + 32;

    let len = Uint256::from_bytes_be(&data[len_start_index..len_end_index]).unwrap();
    let len: usize = match len.try_resize_to_usize() {
        Some(v) => v,
        None => {
            return Err(GravityError::ValidationError(
                "Can't resize to usize".into(),
            ))
        }
    };

    let mut list: Vec<EthAddress> = Vec::new();
    if len == 0 {
        return Ok(list);
    }

    for i in 0..len {
        let start_index = len_end_index + (32 * i);
        let end_index = start_index + 32;

        match EthAddress::from_slice(&data[start_index + 12..end_index]) {
            Ok(v) => list.push(v),
            Err(e) => {
                return Err(GravityError::ValidationError(format!(
                    "Can't convert bytes from {:?} to {:?} to string, err: {:?}",
                    start_index, end_index, e
                )));
            }
        }
    }

    Ok(list)
}

fn parse_uint256_array(data: &[u8], arg_index: usize) -> Result<Vec<Uint256>, GravityError> {
    // fetching the string from the first 32 bytes
    let offset_start = arg_index * 32;
    let offset_end = offset_start + 32;
    let offset = Uint256::from_bytes_be(&data[offset_start..offset_end]).unwrap();

    // parse start and end of the string length
    let len_start_index: usize = match offset.try_resize_to_usize() {
        Some(v) => v,
        None => {
            return Err(GravityError::ValidationError(
                "Can't resize to usize".into(),
            ))
        }
    };
    let len_end_index = len_start_index + 32;

    let len = Uint256::from_bytes_be(&data[len_start_index..len_end_index]).unwrap();
    let len: usize = match len.try_resize_to_usize() {
        Some(v) => v,
        None => {
            return Err(GravityError::ValidationError(
                "Can't resize to usize".into(),
            ))
        }
    };

    let mut list: Vec<Uint256> = Vec::new();
    if len == 0 {
        return Ok(list);
    }

    for i in 0..len {
        let start_index = len_end_index + (32 * i);
        let end_index = start_index + 32;

        match Uint256::from_bytes_be(&data[start_index..end_index]) {
            Some(v) => list.push(v),
            None => {
                return Err(GravityError::ValidationError(format!(
                    "Can't convert bytes from {:?} to {:?} to Uint256, values is empty",
                    start_index, end_index
                )));
            }
        }
    }

    Ok(list)
}

/// Function used for debug printing hex dumps
/// of ethereum events with each uint256 on a new
/// line
fn _debug_print_data(input: &[u8]) {
    let count = input.len() / 32;
    println!("data hex dump");
    for i in 0..count {
        println!("0x{}", bytes_to_hex_str(&input[(i * 32)..((i * 32) + 32)]))
    }
    println!("end dump");
}

#[cfg(test)]
mod tests {
    use clarity::{u256, utils::hex_str_to_bytes};
    use rand::{
        distributions::{Distribution, Uniform},
        prelude::ThreadRng,
        thread_rng, Rng,
    };

    use super::*;

    const FUZZ_TIMES: u64 = 1_000;

    fn get_fuzz_bytes(rng: &mut ThreadRng) -> Vec<u8> {
        let range = Uniform::from(1..200_000);
        let size: usize = range.sample(rng);
        let event_bytes: Vec<u8> = (0..size)
            .map(|_| {
                let val: u8 = rng.gen();
                val
            })
            .collect();
        event_bytes
    }

    #[test]
    fn test_valset_decode() {
        let event = "0x\
        0000000000000000000000000000000000000000000000000000000000000002\
        00000000000000000000000000000000000000000000000000000000004c4b40\
        00000000000000000000000000000000000000000000000000000000000000c0\
        0000000000000000000000000000000000000000000000000000000000000100\
        0000000000000000000000000000000000000000000000000000000000000160\
        00000000000000000000000000000000000000000000000000000000000001e0\
        0000000000000000000000000000000000000000000000000000000000000005\
        7561746f6d000000000000000000000000000000000000000000000000000000\
        000000000000000000000000000000000000000000000000000000000000002d\
        636f736d6f73317a6b6c386739766436327830796b767771346d646361656879\
        6476776338796c683670616e7000000000000000000000000000000000000000\
        0000000000000000000000000000000000000000000000000000000000000003\
        000000000000000000000000c783df8a850f42e7f7e57013759c285caa701eb6\
        000000000000000000000000e5904695748fe4a84b40b3fc79de2277660bd1d3\
        000000000000000000000000ead9c93b79ae7c1591b1fb5323bd777e86e150d4\
        0000000000000000000000000000000000000000000000000000000000000003\
        0000000000000000000000000000000000000000000000000000000038e38e36\
        0000000000000000000000000000000000000000000000000000000038e38e3c\
        0000000000000000000000000000000000000000000000000000000038e38e39";

        let event_bytes = hex_str_to_bytes(event).unwrap();

        let correct = ValsetDataBytes {
            event_nonce: 2u8.into(),
            reward_amount: u256!(5000000),
            reward_denom: "uatom".to_string(),
            reward_recipient: "cosmos1zkl8g9vd62x0ykvwq4mdcaehydvwc8ylh6panp".to_string(),
            members: vec![
                ValsetMember {
                    eth_address: "0xc783df8a850f42e7F7e57013759C285caa701eB6"
                        .parse()
                        .unwrap(),
                    power: 954437174,
                },
                ValsetMember {
                    eth_address: "0xE5904695748fe4A84b40b3fc79De2277660BD1D3"
                        .parse()
                        .unwrap(),
                    power: 954437180,
                },
                ValsetMember {
                    eth_address: "0xeAD9C93b79Ae7C1591b1FB5323BD777E86e150d4"
                        .parse()
                        .unwrap(),
                    power: 954437177,
                },
            ],
        };
        let res = ValsetUpdatedEvent::decode_data_bytes(&event_bytes).unwrap();
        assert_eq!(correct, res);
    }

    #[test]
    fn test_send_to_cosmos_decode() {
        let event = "0x0000000000000000000000000000000000000000000000000000000000000060\
        0000000000000000000000000000000000000000000000000000000000000064\
        0000000000000000000000000000000000000000000000000000000000000002\
        000000000000000000000000000000000000000000000000000000000000002f\
        67726176697479313139347a613679766737646a7a33633676716c63787a7877\
        636a6b617a397264717332656739700000000000000000000000000000000000";
        let event_bytes = hex_str_to_bytes(event).unwrap();

        let correct = SendToCosmosEventData {
            destination: "gravity1194za6yvg7djz3c6vqlcxzxwcjkaz9rdqs2eg9p".to_string(),
            amount: u256!(100),
            event_nonce: u256!(2),
        };
        let res = SendToCosmosEvent::decode_data_bytes(&event_bytes).unwrap();
        assert_eq!(correct, res);
    }

    #[test]
    fn fuzz_send_to_cosmos_decode() {
        let mut rng = thread_rng();
        for _ in 0..FUZZ_TIMES {
            let event_bytes = get_fuzz_bytes(&mut rng);

            let res = SendToCosmosEvent::decode_data_bytes(&event_bytes);
            match res {
                Ok(_) => println!("Got valid output, this should happen very rarely!"),
                Err(_e) => {}
            }
        }
    }

    #[test]
    fn fuzz_valset_updated_event_decode() {
        let mut rng = thread_rng();
        for _ in 0..FUZZ_TIMES {
            let event_bytes = get_fuzz_bytes(&mut rng);

            let res = ValsetUpdatedEvent::decode_data_bytes(&event_bytes);
            match res {
                Ok(_) => println!("Got valid output, this should happen very rarely!"),
                Err(_e) => {}
            }
        }
    }

    #[test]
    fn fuzz_erc20_deployed_event_decode() {
        let mut rng = thread_rng();
        for _ in 0..FUZZ_TIMES {
            let event_bytes = get_fuzz_bytes(&mut rng);

            let res = Erc20DeployedEvent::decode_data_bytes(&event_bytes);
            match res {
                Ok(_) => println!("Got valid output, this should happen very rarely!"),
                Err(_e) => {}
            }
        }
    }
}
