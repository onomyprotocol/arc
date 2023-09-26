use std::time::Duration;

use onomy_test_lib::{
    cosmovisor::{
        cosmovisor_get_addr, fast_block_times, force_chain_id, force_chain_id_no_genesis,
        set_minimum_gas_price, sh_cosmovisor, sh_cosmovisor_no_dbg,
    },
    reprefix_bech32,
    super_orchestrator::{
        get_separated_val, sh_no_dbg,
        stacked_errors::{Result, StackableErr},
        Command, FileOptions,
    },
    Args, TIMEOUT,
};
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::sleep;

async fn _unused() {
    sleep(Duration::ZERO).await;
    sleep(TIMEOUT).await;
}

#[rustfmt::skip]
pub const DOWNLOAD_GETH: &str = r#"ADD https://gethstore.blob.core.windows.net/builds/geth-linux-amd64-1.12.0-e501b3b0.tar.gz /tmp/geth.tar.gz
RUN cd /tmp && tar -xvf * && mv /tmp/geth-linux-amd64-1.12.0-e501b3b0/geth /usr/bin/geth

RUN mkdir /resources

"#;

pub async fn build(args: &Args) -> Result<()> {
    // TODO test that this works on Mac M1 and Windows

    // build Golang
    Command::new("go mod vendor", &[])
        .ci_mode(true)
        .cwd("./module")
        .run_to_completion()
        .await
        .stack()?
        .assert_success()
        .stack()?;
    let mut make = Command::new("make build-linux-amd64", &[])
        .ci_mode(true)
        .cwd("./module");
    make.envs = vec![
        ("LEDGER_ENABLED".to_string(), "false".to_string()),
        (
            "BIN_PATH".to_owned(),
            "./../tests/dockerfiles/dockerfile_resources/".to_owned(),
        ),
    ];
    make.run_to_completion()
        .await
        .stack()?
        .assert_success()
        .stack()?;

    // build NPM artifacts

    if !args.skip_npm {
        let mut npm = Command::new("npm ci", &[]).ci_mode(true).cwd("./solidity");
        npm.envs = vec![("HUSKY_SKIP_INSTALL".to_string(), "1".to_string())];
        npm.run_to_completion()
            .await
            .stack()?
            .assert_success()
            .stack()?;

        Command::new("npm run typechain", &[])
            .cwd("./solidity")
            .run_to_completion()
            .await
            .stack()?
            .assert_success()
            .stack()?;
    }

    Ok(())
}

/// Information needed for `collect-gentx`
#[derive(Serialize, Deserialize)]
pub struct GentxInfo {
    pub gentx_tar: String,
    // `Value` has `WontImplement` serialization
    pub accounts: String,
    pub balances: String,
}

// NOTE: this uses the local tendermint consAddr for the bridge power
pub async fn gravity_standalone_presetup(daemon_home: &str) -> Result<GentxInfo> {
    let chain_id = "gravity";
    sh_cosmovisor("config chain-id", &[chain_id])
        .await
        .stack()?;
    sh_cosmovisor("config keyring-backend test", &[])
        .await
        .stack()?;
    sh_cosmovisor_no_dbg("init --overwrite", &[chain_id])
        .await
        .stack()?;

    force_chain_id_no_genesis(daemon_home, chain_id)
        .await
        .stack()?;
    fast_block_times(daemon_home).await.stack()?;
    set_minimum_gas_price(daemon_home, "1footoken")
        .await
        .stack()?;

    // we need the stderr to get the mnemonic
    let comres = Command::new("cosmovisor run keys add validator", &[])
        .run_to_completion()
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let _mnemonic = comres
        .stderr_as_utf8()
        .stack()?
        .trim()
        .lines()
        .last()
        .stack_err(|| "no last line")?
        .trim()
        .to_owned();

    let comres = Command::new("cosmovisor run keys add orchestrator", &[])
        .run_to_completion()
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let _mnemonic = comres
        .stderr_as_utf8()
        .stack()?
        .trim()
        .lines()
        .last()
        .stack_err(|| "no last line")?
        .trim()
        .to_owned();

    let allocation = "100000000000000000000000stake,100000000000000000000000footoken,100000000000000000000000ibc/nometadatatoken";
    // TODO for unknown reasons, add-genesis-account cannot find the keys
    let addr = cosmovisor_get_addr("validator").await.stack()?;
    sh_cosmovisor("add-genesis-account", &[&addr, allocation])
        .await
        .stack()?;

    let orch_addr = &cosmovisor_get_addr("orchestrator").await.stack()?;
    sh_cosmovisor("add-genesis-account", &[&orch_addr, allocation])
        .await
        .stack()?;

    let eth_keys = sh_cosmovisor("eth_keys add", &[]).await.stack()?;
    let eth_addr = &get_separated_val(&eth_keys, "\n", "address", ":").stack()?;

    //let consaddr = sh_cosmovisor("tendermint show-address", &[]).await?;
    //let consaddr = consaddr.trim();

    sh_cosmovisor(
        "gentx",
        &[
            "validator",
            "1000000000000000000000stake",
            eth_addr,
            orch_addr,
            "--chain-id",
            chain_id,
            "--min-self-delegation",
            "1",
        ],
    )
    .await
    .stack()?;

    // this is not the multi validator genesis, but `add-genesis-account` added some information that we need
    let genesis_s = FileOptions::read_to_string(&format!("{daemon_home}/config/genesis.json"))
        .await
        .stack()?;
    let genesis: Value = serde_json::from_str(&genesis_s).stack()?;
    let accounts = genesis["app_state"]["auth"]["accounts"].to_string();
    let balances = genesis["app_state"]["bank"]["balances"].to_string();

    let gentx_tar = sh_no_dbg(
        &format!("tar --create --to-stdout {daemon_home}/config/gentx"),
        &[],
    )
    .await
    .stack()?;

    Ok(GentxInfo {
        gentx_tar,
        accounts,
        balances,
    })
}

/// Assembles all the gentxs together and makes the genesis file changes
pub async fn gravity_standalone_central_setup(
    daemon_home: &str,
    chain_id: &str,
    address_prefix: &str,
) -> Result<String> {
    let genesis_file_path = format!("{daemon_home}/config/genesis.json");
    let genesis_s = FileOptions::read_to_string(&genesis_file_path)
        .await
        .stack()?;
    let mut genesis: Value = serde_json::from_str(&genesis_s).stack()?;

    force_chain_id(daemon_home, &mut genesis, chain_id)
        .await
        .stack()?;

    let denom_metadata = json!([
        {"name": "Foo Token", "symbol": "FOO", "base": "footoken", "display": "mfootoken",
        "description": "Foo token", "denom_units": [{"denom": "footoken", "exponent": 0},
        {"denom": "mfootoken", "exponent": 6}]},
        {"name": "Stake Token", "symbol": "STAKE", "base": "stake", "display": "mstake",
        "description": "Staking token", "denom_units": [{"denom": "stake", "exponent": 0},
        {"denom": "mstake", "exponent": 6}]}
    ]);
    genesis["app_state"]["bank"]["denom_metadata"] = denom_metadata;

    // for airdrop tests
    genesis["app_state"]["distribution"]["fee_pool"]["community_pool"] = json!(
        [{"denom": "stake", "amount": "10000000000.0"}]
    );
    // SHA256 hash of distribution.ModuleName
    let distribution_addr = reprefix_bech32(
        "gravity1jv65s3grqf6v6jl3dp4t6c9t9rk99cd8r0kyvh",
        address_prefix,
    )
    .unwrap();
    // normally this would be generated after genesis, but we need to set this manually before hand so that it starts with the desired balance
    genesis["app_state"]["auth"]["accounts"]
        .as_array_mut()
        .unwrap()
        .push(json!(
            {"@type": "/cosmos.auth.v1beta1.ModuleAccount",
            "base_account": { "account_number": "0", "address": distribution_addr,
            "pub_key": null,"sequence": "0"},
            "name": "distribution", "permissions": ["basic"]}
        ));
    genesis["app_state"]["bank"]["balances"]
        .as_array_mut()
        .unwrap()
        .push(json!(
            {"address": distribution_addr, "coins": [{"amount": "10000000000", "denom": "stake"}]}
        ));

    // short but not too short governance period
    let gov_period = "30s";
    let gov_period: Value = gov_period.into();
    genesis["app_state"]["gov"]["voting_params"]["voting_period"] = gov_period.clone();
    genesis["app_state"]["gov"]["deposit_params"]["max_deposit_period"] = gov_period;

    // write back genesis
    let genesis_s = serde_json::to_string(&genesis).stack()?;
    FileOptions::write_str(&genesis_file_path, &genesis_s)
        .await
        .stack()?;

    sh_cosmovisor_no_dbg("collect-gentxs", &[]).await.stack()?;

    let complete_genesis = FileOptions::read_to_string(&genesis_file_path)
        .await
        .stack()?;

    FileOptions::write_str(&format!("/logs/{chain_id}_genesis.json"), &complete_genesis)
        .await
        .stack()?;

    Ok(complete_genesis)
}
