use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::{clarity::Address as EthAddress, deep_space::Contact, web30::client::Web3};
use tonic::transport::Channel;

use crate::{
    utils::{create_default_test_config, start_orchestrators, ValidatorKeys},
    validator_out::test_valset_update,
};

pub async fn validator_set_stress_test(
    web30: &Web3,
    grpc_client: GravityQueryClient<Channel>,
    contact: &Contact,
    keys: Vec<ValidatorKeys>,
    gravity_address: EthAddress,
) {
    let mut grpc_client = grpc_client.clone();
    let no_relay_market_config = create_default_test_config();
    start_orchestrators(keys.clone(), gravity_address, false, no_relay_market_config).await;

    for _ in 0u32..10 {
        test_valset_update(web30, contact, &mut grpc_client, &keys, gravity_address).await;
    }
}
