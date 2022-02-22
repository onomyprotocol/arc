use gravity_proto::gravity::{
    query_client::QueryClient as GravityQueryClient, OutgoingLogicCall as ProtoLogicCall,
    OutgoingTxBatch as ProtoBatch, Valset as ProtoValset,
};
use gravity_utils::{
    deep_space::{utils::encode_any, Address as CosmosAddress},
    get_with_retry::RETRY_TIME,
    types::{LogicCall, TransactionBatch, Valset},
};
use prost_types::Any;
use tokio::time::sleep;
use tonic::transport::Channel;

use crate::query::get_last_event_nonce_for_validator;

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
