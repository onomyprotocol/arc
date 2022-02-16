use std::{convert::TryInto, fs, process::exit};

use cosmos_gravity::proposals::{
    submit_airdrop_proposal, submit_ibc_metadata_proposal, submit_pause_bridge_proposal,
    submit_unhalt_bridge_proposal, AirdropProposalJsonUnparsed, IbcMetadataProposalJson,
    PauseBridgeProposalJson, UnhaltBridgeProposalJson,
};
use gravity_utils::connection_prep::create_rpc_connections;

use crate::{
    args::{
        AirdropProposalOpts, EmergencyBridgeHaltProposalOpts, IbcMetadataProposalOpts,
        OracleUnhaltProposalOpts,
    },
    utils::TIMEOUT,
};

pub async fn submit_ibc_metadata(opts: IbcMetadataProposalOpts, prefix: String) {
    let connections = create_rpc_connections(prefix, Some(opts.cosmos_grpc), None, TIMEOUT).await;
    let contact = connections.contact.unwrap();

    match fs::read_to_string(opts.json) {
        Ok(file_contents) => {
            let proposal: Result<IbcMetadataProposalJson, _> = serde_json::from_str(&file_contents);
            match proposal {
                Ok(proposal_json) => {
                    let res = submit_ibc_metadata_proposal(
                        proposal_json.into(),
                        opts.deposit,
                        opts.fees,
                        &contact,
                        opts.cosmos_phrase,
                        Some(TIMEOUT),
                    )
                    .await;
                    match res {
                        Ok(r) => info!("Successfully submitted proposal with txid {}", r.txhash),
                        Err(e) => {
                            error!("Failed to submit proposal with {:?}", e);
                            exit(1);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to deserialize your proposal.json, check the contents! {:?}",
                        e
                    );
                    exit(1);
                }
            }
        }
        Err(e) => {
            error!(
                "Failed to read your proposal.json check the file path! {:?}",
                e
            );
            exit(1);
        }
    }
}

pub async fn submit_airdrop(opts: AirdropProposalOpts, prefix: String) {
    let connections = create_rpc_connections(prefix, Some(opts.cosmos_grpc), None, TIMEOUT).await;
    let contact = connections.contact.unwrap();

    match fs::read_to_string(opts.json) {
        Ok(file_contents) => {
            let proposal: Result<AirdropProposalJsonUnparsed, _> =
                serde_json::from_str(&file_contents);
            match proposal {
                Ok(proposal_json) => {
                    let res = submit_airdrop_proposal(
                        proposal_json
                            .try_into()
                            .expect("Invalid address in proposal.json"),
                        opts.deposit,
                        opts.fees,
                        &contact,
                        opts.cosmos_phrase,
                        Some(TIMEOUT),
                    )
                    .await;
                    match res {
                        Ok(r) => info!("Successfully submitted proposal with txid {}", r.txhash),
                        Err(e) => {
                            error!("Failed to submit proposal with {:?}", e);
                            exit(1);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to deserialize your proposal.json, check the contents! {:?}",
                        e
                    );
                    exit(1);
                }
            }
        }
        Err(e) => {
            error!(
                "Failed to read your proposal.json check the file path! {:?}",
                e
            );
            exit(1);
        }
    }
}

pub async fn submit_emergency_bridge_halt(opts: EmergencyBridgeHaltProposalOpts, prefix: String) {
    let connections = create_rpc_connections(prefix, Some(opts.cosmos_grpc), None, TIMEOUT).await;
    let contact = connections.contact.unwrap();

    match fs::read_to_string(opts.json) {
        Ok(file_contents) => {
            let proposal: Result<PauseBridgeProposalJson, _> = serde_json::from_str(&file_contents);
            match proposal {
                Ok(proposal_json) => {
                    let res = submit_pause_bridge_proposal(
                        proposal_json,
                        opts.deposit,
                        opts.fees,
                        &contact,
                        opts.cosmos_phrase,
                        Some(TIMEOUT),
                    )
                    .await;
                    match res {
                        Ok(r) => info!("Successfully submitted proposal with txid {}", r.txhash),
                        Err(e) => {
                            error!("Failed to submit proposal with {:?}", e);
                            exit(1);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to deserialize your proposal.json, check the contents! {:?}",
                        e
                    );
                    exit(1);
                }
            }
        }
        Err(e) => {
            error!(
                "Failed to read your proposal.json check the file path! {:?}",
                e
            );
            exit(1);
        }
    }
}

pub async fn submit_oracle_unhalt(opts: OracleUnhaltProposalOpts, prefix: String) {
    let connections = create_rpc_connections(prefix, Some(opts.cosmos_grpc), None, TIMEOUT).await;
    let contact = connections.contact.unwrap();

    match fs::read_to_string(opts.json) {
        Ok(file_contents) => {
            let proposal: Result<UnhaltBridgeProposalJson, _> =
                serde_json::from_str(&file_contents);
            match proposal {
                Ok(proposal_json) => {
                    let res = submit_unhalt_bridge_proposal(
                        proposal_json.into(),
                        opts.deposit,
                        opts.fees,
                        &contact,
                        opts.cosmos_phrase,
                        Some(TIMEOUT),
                    )
                    .await;
                    match res {
                        Ok(r) => info!("Successfully submitted proposal with txid {}", r.txhash),
                        Err(e) => {
                            error!("Failed to submit proposal with {:?}", e);
                            exit(1);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to deserialize your proposal.json, check the contents! {:?}",
                        e
                    );
                    exit(1);
                }
            }
        }
        Err(e) => {
            error!(
                "Failed to read your proposal.json check the file path! {:?}",
                e
            );
            exit(1);
        }
    }
}
