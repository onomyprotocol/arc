use crate::query::get_last_event_nonce_for_validator;
use deep_space::error::CosmosGrpcError;
use deep_space::utils::encode_any;
use deep_space::{Address as CosmosAddress, Contact};
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_proto::gravity::OutgoingLogicCall as ProtoLogicCall;
use gravity_proto::gravity::OutgoingTxBatch as ProtoBatch;
use gravity_proto::gravity::Valset as ProtoValset;
use gravity_utils::get_with_retry::RETRY_TIME;
use gravity_utils::types::LogicCall;
use gravity_utils::types::TransactionBatch;
use gravity_utils::types::Valset;
use prost_types::Any;
use std::time::Duration;
use tokio::time::sleep;
use tonic::transport::Channel;

pub async fn wait_for_cosmos_online(contact: &Contact, timeout: Duration) {
    match tokio::time::timeout(timeout, contact.wait_for_next_block(timeout)).await {
        Ok(Err(CosmosGrpcError::NodeNotSynced) | Err(CosmosGrpcError::ChainNotRunning)) => {
            panic!("Cosmos node has not come online during timeout!")
        }
        Err(_) => debug!("timedout"),
        _ => {}
    }

    for _ in 0..3 {
        let _ = contact.wait_for_next_block(timeout).await;
    }
}

/// gets the Cosmos last event nonce, no matter how long it takes.
pub async fn get_last_event_nonce_with_retry(
    client: &mut GravityQueryClient<Channel>,
    our_cosmos_address: CosmosAddress,
    prefix: String,
) -> u64 {
    loop {
        match get_last_event_nonce_for_validator(client, our_cosmos_address, prefix.clone()).await {
            Err(res) => {
                error!(
                    "Failed to get last event nonce, is the Cosmos GRPC working? {:?}",
                    res
                );
                sleep(RETRY_TIME).await;
            }
            Ok(last_nonce) => return last_nonce,
        }
    }
}

pub enum BadSignatureEvidence {
    Valset(Valset),
    Batch(TransactionBatch),
    LogicCall(LogicCall),
}

impl BadSignatureEvidence {
    pub fn to_any(&self) -> Any {
        match self {
            BadSignatureEvidence::Valset(v) => {
                let v: ProtoValset = v.into();
                encode_any(v, "/gravity.v1.Valset".to_string())
            }
            BadSignatureEvidence::Batch(b) => {
                let b: ProtoBatch = b.into();
                encode_any(b, "/gravity.v1.OutgoingTxBatch".to_string())
            }
            BadSignatureEvidence::LogicCall(l) => {
                let l: ProtoLogicCall = l.into();
                encode_any(l, "/gravity.v1.OutgoingLogicCall".to_string())
            }
        }
    }
}
