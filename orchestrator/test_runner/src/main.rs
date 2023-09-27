//! This entrypoint is not used by the super_orchestrator tests, but might be used by scripts and manual tests

use std::env;

use test_runner::{
    get_keys, run_test, ADDRESS_PREFIX, COSMOS_NODE_ABCI, COSMOS_NODE_GRPC, ETH_NODE,
};

#[tokio::main]
pub async fn main() {
    env_logger::init();

    if matches!(env::var("GET_TEST_ADDRESS").as_deref(), Ok("1")) {
        // A way to get the correct Bech32 to the scripts. Derived from the
        // sha256 hash of 'distribution' to create the address of the module
        let data = bech32::decode("gravity1jv65s3grqf6v6jl3dp4t6c9t9rk99cd8r0kyvh")
            .unwrap()
            .1;
        println!(
            "{}",
            bech32::encode(&ADDRESS_PREFIX, data, bech32::Variant::Bech32).unwrap()
        );
        return;
    }

    let keys = get_keys();

    run_test(
        COSMOS_NODE_GRPC.as_str(),
        COSMOS_NODE_ABCI.as_str(),
        ETH_NODE.as_str(),
        keys,
    )
    .await;
}
