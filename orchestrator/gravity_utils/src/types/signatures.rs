use std::cmp::Ordering;

use clarity::{abi::Token, Address as EthAddress, Signature as EthSignature, Uint256};
use serde::{Deserialize, Serialize};

/// A sortable struct of a validator and it's signatures
/// this can be used for either transaction batch or validator
/// set signatures
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct GravitySignature {
    pub power: u64,
    pub eth_address: EthAddress,
    pub v: Uint256,
    pub r: Uint256,
    pub s: Uint256,
}

impl Ord for GravitySignature {
    // Sort by eth address asc
    fn cmp(&self, other: &Self) -> Ordering {
        self.eth_address.cmp(&other.eth_address)
    }
}

impl PartialOrd for GravitySignature {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// signatures in array formats ready to be
/// submitted to the Gravity Ethereum Contract
pub struct GravitySignatureArrays {
    pub addresses: Vec<EthAddress>,
    pub powers: Vec<u64>,
    pub sigs: Token,
}

/// This function handles converting the GravitySignature type into an Ethereum
/// submittable signature struct, including the finicky token encoding tricks you need to
/// perform in order to distinguish between a uint8[] and bytes32[]
pub fn to_arrays(input: Vec<GravitySignature>) -> GravitySignatureArrays {
    let mut addresses = Vec::new();
    let mut powers = Vec::new();
    let mut sigs = Vec::new();
    for val in input {
        addresses.push(val.eth_address);
        powers.push(val.power);
        sigs.push(Token::Struct(
            [val.v.into(), val.r.into(), val.s.into()].to_vec(),
        ));
    }
    GravitySignatureArrays {
        addresses,
        powers,
        sigs: Token::Dynamic(sigs),
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct SigWithAddress {
    pub eth_address: EthAddress,
    pub eth_signature: EthSignature,
}

#[cfg(test)]
mod tests {
    use clarity::u256;
    use rand::{seq::SliceRandom, thread_rng};

    use super::*;

    #[test]
    fn test_valset_sort() {
        let correct: [GravitySignature; 8] = [
            GravitySignature {
                power: 678509841,
                eth_address: "0x0A7254b318dd742A3086882321C27779B4B642a6"
                    .parse()
                    .unwrap(),
                v: u256!(0),
                r: u256!(0),
                s: u256!(0),
            },
            GravitySignature {
                power: 685294939,
                eth_address: "0x3511A211A6759d48d107898302042d1301187BA9"
                    .parse()
                    .unwrap(),
                v: u256!(0),
                r: u256!(0),
                s: u256!(0),
            },
            GravitySignature {
                power: 678509841,
                eth_address: "0x37A0603dA2ff6377E5C7f75698dabA8EE4Ba97B8"
                    .parse()
                    .unwrap(),
                v: u256!(0),
                r: u256!(0),
                s: u256!(0),
            },
            GravitySignature {
                power: 671724742,
                eth_address: "0x454330deAaB759468065d08F2b3B0562caBe1dD1"
                    .parse()
                    .unwrap(),
                v: u256!(0),
                r: u256!(0),
                s: u256!(0),
            },
            GravitySignature {
                power: 671724742,
                eth_address: "0x479FFc856Cdfa0f5D1AE6Fa61915b01351A7773D"
                    .parse()
                    .unwrap(),
                v: u256!(0),
                r: u256!(0),
                s: u256!(0),
            },
            GravitySignature {
                power: 617443955,
                eth_address: "0xa14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"
                    .parse()
                    .unwrap(),
                v: u256!(0),
                r: u256!(0),
                s: u256!(0),
            },
            GravitySignature {
                power: 291759231,
                eth_address: "0xA24879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"
                    .parse()
                    .unwrap(),
                v: u256!(0),
                r: u256!(0),
                s: u256!(0),
            },
            GravitySignature {
                power: 6785098,
                eth_address: "0xF14879a175A2F1cEFC7c616f35b6d9c2b0Fd8326"
                    .parse()
                    .unwrap(),
                v: u256!(0),
                r: u256!(0),
                s: u256!(0),
            },
        ];
        let mut rng = thread_rng();
        let mut incorrect = correct;

        incorrect.shuffle(&mut rng);
        assert_ne!(incorrect, correct);

        incorrect.sort();
        assert_eq!(incorrect, correct);
    }
}
