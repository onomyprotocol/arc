use std::time::Duration;

use common::DOWNLOAD_GETH;
use gravity_utils::web30::client::Web3;
use log::info;
use onomy_test_lib::{
    cosmovisor::{cosmovisor_start, CosmovisorOptions},
    dockerfiles::{onomy_std_cosmos_daemon, ONOMY_STD},
    onomy_std_init,
    setups::gravity_standalone_setup,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        net_message::NetMessenger,
        sh,
        stacked_errors::{Error, Result, StackableErr},
        wait_for_ok, Command, FileOptions, STD_DELAY, STD_TRIES,
    },
    Args, TIMEOUT,
};
use test_runner::{run_test, ADDRESS_PREFIX};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    let num_nodes = 4;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "geth" => geth_runner().await,
            "test" => test_runner(num_nodes).await,
            "validator" => cosmos_validator(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        // TODO test that this works on Mac M1 and Windows

        // build Golang
        let mut vendor = Command::new("go mod vendor", &[]).ci_mode(true);
        vendor.cwd = Some("./module".to_owned());
        let comres = vendor.run_to_completion().await.stack()?;
        comres.assert_success()?;
        let mut make = Command::new("make build-linux-amd64", &[]).ci_mode(true);
        make.cwd = Some("./module".to_owned());
        make.envs = vec![
            ("LEDGER_ENABLED".to_string(), "false".to_string()),
            (
                "BIN_PATH".to_owned(),
                "./../tests/dockerfiles/dockerfile_resources/".to_owned(),
            ),
        ];
        let comres = make.run_to_completion().await.stack()?;
        comres.assert_success()?;

        // build NPM artifacts

        if !args.skip_npm {
            let mut npm = Command::new("npm ci", &[]).ci_mode(true);
            npm.cwd = Some("./solidity".to_owned());
            npm.envs = vec![("HUSKY_SKIP_INSTALL".to_string(), "1".to_string())];
            let comres = npm.run_to_completion().await.stack()?;
            comres.assert_success()?;

            let mut typechain = Command::new("npm run typechain", &[]);
            typechain.cwd = Some("./solidity".to_owned());
            let comres = typechain.run_to_completion().await.stack()?;
            comres.assert_success()?;
        }

        container_runner(&args, num_nodes).await
    }
}

async fn container_runner(args: &Args, num_nodes: u64) -> Result<()> {
    let logs_dir = "./tests/logs";
    let dockerfiles_dir = "./tests/dockerfiles";
    let bin_entrypoint = &args.bin_name;
    let container_target = "x86_64-unknown-linux-gnu";

    // build internal runner with `--release`
    sh(
        "cargo build --release --bin",
        &[bin_entrypoint, "--target", container_target],
    )
    .await
    .stack()?;

    let entrypoint = Some(format!(
        "./target/{container_target}/release/{bin_entrypoint}"
    ));
    let entrypoint = entrypoint.as_deref();

    let mut containers = vec![
        Container::new(
            "geth",
            Dockerfile::Contents(format!("{ONOMY_STD} {DOWNLOAD_GETH}")),
            entrypoint,
            &["--entry-name", "geth"],
        ),
        Container::new(
            "test",
            Dockerfile::Contents(ONOMY_STD.to_owned()),
            entrypoint,
            &["--entry-name", "test"],
        ),
        // may want prometheus crate for Rust
        /*Container::new(
            "prometheus",
            Dockerfile::NameTag("prom/prometheus:v2.44.0".to_owned()),
            None,
            &[],
        )
        .create_args(&["-p", "9090:9090"]),*/
    ];
    for i in 0..num_nodes {
        containers.push(Container::new(
            &format!("validator{i}"),
            Dockerfile::Contents(onomy_std_cosmos_daemon(
                "gravityd", ".gravity", "v0.1.0", "gravityd",
            )),
            entrypoint,
            &["--entry-name", "validator", "--i", &format!("{i}")],
        ));
    }

    let mut cn = ContainerNetwork::new("test", containers, Some(dockerfiles_dir), true, logs_dir)
        .stack()?
        .add_common_volumes(&[(logs_dir, "/logs")]);
    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
    Ok(())
}

async fn test_runner(num_nodes: u64) -> Result<()> {
    let mut nm_geth = NetMessenger::connect(STD_TRIES, STD_DELAY, "geth:26000")
        .await
        .stack()?;

    let mut nm_validators = vec![];
    for i in 0..num_nodes {
        nm_validators.push(
            NetMessenger::connect(STD_TRIES, STD_DELAY, &format!("validator{i}:26000"))
                .await
                .stack()?,
        );
    }

    // manual HTTP request
    /*
    curl --header "content-type: application/json" --data
    '{"id":1,"jsonrpc":"2.0","method":"eth_syncing","params":[]}' http://geth:8545
    */

    // programmatic HTTP request
    /*
    sleep(Duration::from_secs(5)).await;
    let geth_client = reqwest::Client::new();
    let res = geth_client
        .post("http://geth:8545")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/json",
        )
        .body(r#"{"method":"eth_blockNumber","params":[],"id":1,"jsonrpc":"2.0"}"#)
        .send()
        .await.stack()?
        .text()
        .await.stack()?;
    info!(res);
    */

    // requests using the `web30` crate
    let web3 = Web3::new("http://geth:8545", Duration::from_secs(30));
    // `Web3::new` only waits for initial handshakes, we need to wait for Tcp and
    // syncing
    async fn is_eth_up(web3: &Web3) -> Result<()> {
        web3.eth_syncing().await.map(|_| ()).stack()
    }
    wait_for_ok(STD_TRIES, STD_DELAY, || is_eth_up(&web3))
        .await
        .stack()?;
    info!("geth is running");

    run_test().await;

    // terminate
    for mut nm in nm_validators {
        nm.send::<()>(&()).await.stack()?;
    }
    nm_geth.send::<()>(&()).await.stack()?;

    Ok(())
}

#[rustfmt::skip]
const ETH_GENESIS: &str = r#"
{
    "config": {
      "chainId": 15,
      "homesteadBlock": 0,
      "eip150Block": 0,
      "eip155Block": 0,
      "eip158Block": 0,
      "byzantiumBlock": 0,
      "constantinopleBlock": 0,
      "petersburgBlock": 0,
      "istanbulBlock": 0,
      "berlinBlock": 0,
      "clique": {
        "period": 1,
        "epoch": 30000
      }
    },
    "difficulty": "1",
    "gasLimit": "8000000",
    "extradata": "0x0000000000000000000000000000000000000000000000000000000000000000Bf660843528035a5A4921534E156a27e64B231fE0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "alloc": {
      "0xBf660843528035a5A4921534E156a27e64B231fE": { "balance": "0x1337000000000000000000" }
    }
}
"#;

async fn geth_runner() -> Result<()> {
    let mut nm_test = NetMessenger::listen_single_connect("0.0.0.0:26000", TIMEOUT)
        .await
        .stack()?;

    let genesis_file = "/resources/eth_genesis.json";
    FileOptions::write_str(genesis_file, ETH_GENESIS)
        .await
        .stack()?;

    // the private key must not have the leading "0x"
    let private_key_no_0x = "b1bab011e03a9862664706fc3bbaa1b16651528e5f0e7fbfcbfdd8be302a13e7";
    let private_key_path = "/resources/test_private_key.txt";
    let test_password = "testpassword";
    let test_password_path = "/resources/test_password.txt";
    FileOptions::write_str(private_key_path, private_key_no_0x)
        .await
        .stack()?;
    FileOptions::write_str(test_password_path, test_password)
        .await
        .stack()?;

    sh(
        "geth account import --password",
        &[test_password_path, private_key_path],
    )
    .await
    .stack()?;

    sh(
        "geth --identity \"testnet\" --networkid 15 init",
        &[genesis_file],
    )
    .await
    .stack()?;

    let geth_log = FileOptions::write2("/logs", "geth_runner.log");
    let mut geth_runner = Command::new(
        "geth",
        &[
            "--nodiscover",
            "--allow-insecure-unlock",
            "--unlock",
            "0xBf660843528035a5A4921534E156a27e64B231fE",
            "--password",
            test_password_path,
            "--mine",
            "--miner.etherbase",
            "0xBf660843528035a5A4921534E156a27e64B231fE",
            "--http",
            "--http.addr",
            "0.0.0.0",
            "--http.vhosts",
            "*",
            "--http.corsdomain",
            "*",
            "--nousb",
            "--verbosity",
            "4",
            // TODO --metrics.
        ],
    )
    .stderr_log(&geth_log)
    .stdout_log(&geth_log)
    .run()
    .await
    .stack()?;

    // terminate
    nm_test.recv::<()>().await.stack()?;

    geth_runner.terminate().await.stack()?;
    Ok(())
}

async fn cosmos_validator(args: &Args) -> Result<()> {
    let mut nm_test = NetMessenger::listen_single_connect("0.0.0.0:26000", TIMEOUT)
        .await
        .stack()?;

    let daemon_home = args.daemon_home.as_ref().stack()?;
    let validator_i = args.i.stack()?;
    gravity_standalone_setup(daemon_home, false, ADDRESS_PREFIX.as_str())
        .await
        .stack()?;
    let options = CosmovisorOptions::default();
    //options.
    sleep(TIMEOUT).await;
    let mut cosmovisor_runner =
        cosmovisor_start(&format!("gravity_runner{validator_i}.log"), Some(options))
            .await
            .stack()?;

    // terminate
    nm_test.recv::<()>().await.stack()?;
    sleep(Duration::ZERO).await;
    cosmovisor_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
