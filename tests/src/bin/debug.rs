//! Used for manual debugging

use std::time::Duration;

use common::DOWNLOAD_GETH;
use gravity_utils::web30::client::Web3;
use log::info;
use onomy_test_lib::{
    dockerfiles::{onomy_std_cosmos_daemon, ONOMY_STD},
    onomy_std_init,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Error, Result, StackableErr},
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

async fn _unused() -> Result<()> {
    let _ = DOWNLOAD_GETH;
    let _ = ONOMY_STD;
    sleep(TIMEOUT).await;
    sleep(Duration::ZERO).await;
    let _ = Web3::new("", Duration::ZERO);
    Err(Error::from("")).stack()?;
    info!("");
    Ok(())
}

/*
#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    let url = "http://192.168.208.3:8545";

    // manual HTTP request
    /*
    curl --header "content-type: application/json" --data
    '{"id":1,"jsonrpc":"2.0","method":"eth_syncing","params":[]}' http://geth:8545
    */

    // reqwest
    /*
    sleep(Duration::from_secs(5)).await;
    let geth_client = reqwest::Client::new();
    let res = geth_client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(r#"{"method":"eth_blockNumber","params":[],"id":1,"jsonrpc":"2.0"}"#)
        .send()
        .await
        .stack()?
        .text()
        .await
        .stack()?;
    info!("{res}");
    */

    // web3 crate
    let web3 = Web3::new(url, Duration::from_secs(30));
    dbg!(web3.eth_syncing().await.stack()?);

    Ok(())
}
*/

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "test" => test_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        container_runner(&args).await
    }
}

async fn container_runner(args: &Args) -> Result<()> {
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

    let containers = vec![
        /*Container::new(
            "geth",
            Dockerfile::Contents(format!("{ONOMY_STD} {DOWNLOAD_GETH}")),
            entrypoint,
            &["--entry-name", "geth"],
        ),*/
        Container::new(
            "debug_test",
            // this is used just for the common gentx
            Dockerfile::Contents(onomy_std_cosmos_daemon(
                "gravityd", ".gravity", "v0.1.0", "gravityd",
            )),
            entrypoint,
            &["--entry-name", "test"],
        ),
    ];

    let mut cn =
        ContainerNetwork::new("test", containers, Some(dockerfiles_dir), true, logs_dir).stack()?;
    cn.add_common_volumes(&[(logs_dir, "/logs")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(&["--uuid", &uuid]);

    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
    cn.terminate_all().await;
    Ok(())
}

async fn test_runner(_args: &Args) -> Result<()> {
    let url = "http://192.168.208.3:8545";

    // web3 crate
    let web3 = Web3::new(url, Duration::from_secs(5));
    dbg!(web3.eth_syncing().await.stack()?);

    Ok(())
}
