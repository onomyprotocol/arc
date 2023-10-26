#[macro_use]
extern crate log;

use std::process::exit;

use args::{GovQuerySubcommand, GovSubcommand, GovSubmitSubcommand, Opts};
use clap::Parser;
use client::{
    cosmos_to_eth::cosmos_to_eth, deploy_erc20_representation::deploy_erc20_representation,
    eth_to_cosmos::eth_to_cosmos,
};
use config::{get_home_dir, load_config};
use env_logger::Env;
use gov::{
    proposals::{
        submit_airdrop, submit_emergency_bridge_halt, submit_ibc_metadata, submit_oracle_unhalt,
    },
    queries::query_airdrops,
};
use gravity_utils::{
    error::GravityError,
    stacked_errors::{Error, ErrorKind, StackableErr},
};
use keys::register_orchestrator_address::register_orchestrator_address;

use crate::{
    args::{ClientSubcommand, KeysSubcommand, SubCommand},
    config::init_config,
    orchestrator::orchestrator,
    relayer::relayer,
};

mod args;
mod client;
mod config;
mod gov;
mod keys;
mod orchestrator;
mod relayer;
mod utils;

#[allow(clippy::single_match, clippy::collapsible_match)]
#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    // On Linux static builds we need to probe ssl certs path to be able to
    // do TLS stuff.
    openssl_probe::init_ssl_cert_env_vars();

    if let Err(error) = run_gbt().await {
        let mut should_exit = false;
        for (suberror, _) in &error.stack {
            match suberror {
                ErrorKind::BoxedError(boxed_error) => {
                    if let Some(gravity_error) = boxed_error.downcast_ref::<GravityError>() {
                        match gravity_error {
                            // exit only on unrecoverable errors
                            GravityError::UnrecoverableError(_) => {
                                should_exit = true;
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            }
        }
        error!("{}", error);
        if should_exit {
            exit(1);
        }
    }
}

async fn run_gbt() -> Result<(), Error> {
    // parse the arguments
    let opts: Opts = Opts::parse();

    // handle global config here
    let address_prefix = opts.address_prefix;
    let home_dir = get_home_dir(opts.home).stack()?;
    let config = load_config(&home_dir).stack()?;

    // control flow for the command structure
    match opts.subcmd {
        SubCommand::Client(client_opts) => match client_opts.subcmd {
            ClientSubcommand::EthToCosmos(eth_to_cosmos_opts) => {
                eth_to_cosmos(eth_to_cosmos_opts, address_prefix)
                    .await
                    .stack()
            }
            ClientSubcommand::CosmosToEth(cosmos_to_eth_opts) => {
                cosmos_to_eth(cosmos_to_eth_opts, address_prefix)
                    .await
                    .stack()
            }
            ClientSubcommand::DeployErc20Representation(deploy_erc20_opts) => {
                deploy_erc20_representation(deploy_erc20_opts, address_prefix)
                    .await
                    .stack()
            }
        },
        SubCommand::Keys(key_opts) => match key_opts.subcmd {
            KeysSubcommand::RegisterOrchestratorAddress(set_orchestrator_address_opts) => {
                register_orchestrator_address(set_orchestrator_address_opts, address_prefix)
                    .await
                    .stack()
            }
        },
        SubCommand::Orchestrator(orchestrator_opts) => {
            orchestrator(orchestrator_opts, address_prefix, config)
                .await
                .stack()
        }
        SubCommand::Relayer(relayer_opts) => relayer(relayer_opts, address_prefix, &config.relayer)
            .await
            .stack(),
        SubCommand::Init(init_opts) => init_config(init_opts, home_dir).stack(),
        SubCommand::Gov(gov_opts) => match gov_opts.subcmd {
            GovSubcommand::Submit(submit_opts) => match submit_opts {
                GovSubmitSubcommand::IbcMetadata(opts) => {
                    submit_ibc_metadata(opts, address_prefix).await.stack()
                }
                GovSubmitSubcommand::Airdrop(opts) => {
                    submit_airdrop(opts, address_prefix).await.stack()
                }
                GovSubmitSubcommand::EmergencyBridgeHalt(opts) => {
                    submit_emergency_bridge_halt(opts, address_prefix)
                        .await
                        .stack()
                }
                GovSubmitSubcommand::OracleUnhalt(opts) => {
                    submit_oracle_unhalt(opts, address_prefix).await.stack()
                }
            },
            GovSubcommand::Query(query_opts) => match query_opts {
                GovQuerySubcommand::Airdrop(opts) => {
                    query_airdrops(opts, address_prefix).await.stack()
                }
            },
        },
    }
}
