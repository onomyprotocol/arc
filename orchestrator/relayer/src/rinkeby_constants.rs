// Rinkeby interactions with AMMs (Automated Market Makers) on ethereum
use clarity::Address;

lazy_static! {
    /// The Wrapped Ether's address, on rinkeby Ethereum
    pub static ref WETH_CONTRACT_ADDRESS_RINKEBY: Address =
        Address::parse_and_validate("0xc778417E063141139Fce010982780140Aa0cD5Ab").unwrap();
    /// The DAI contract address, on rinkeby Ethereum
    pub static ref DAI_CONTRACT_ADDRESS_RINKEBY: Address =
        Address::parse_and_validate("0x5592EC0cfb4dbc12D3aB100b257153436a1f0FEa").unwrap();
}
