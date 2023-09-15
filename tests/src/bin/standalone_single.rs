use std::time::Duration;

use common::{build, gravity_standalone_setup};
use log::info;
use onomy_test_lib::{
    cosmovisor::{cosmovisor_start, CosmovisorOptions},
    dockerfiles::onomy_std_cosmos_daemon,
    onomy_std_init,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Error, Result, StackableErr},
    },
    Args, TIMEOUT,
};
use test_runner::ADDRESS_PREFIX;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "validator" => cosmos_validator(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        build(&args).await.stack()?;
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

    let mut cn = ContainerNetwork::new(
        "test",
        vec![Container::new(
            &format!("validator_0"),
            Dockerfile::Contents(onomy_std_cosmos_daemon(
                "gravityd", ".gravity", "v0.1.0", "gravityd",
            )),
            entrypoint,
            &["--entry-name", "validator"],
        )],
        Some(dockerfiles_dir),
        true,
        logs_dir,
    )
    .stack()?;
    cn.add_common_volumes(&[(logs_dir, "/logs")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(&["--uuid", &uuid]);

    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
    cn.terminate_all().await;
    Ok(())
}

async fn cosmos_validator(args: &Args) -> Result<()> {
    let daemon_home = args.daemon_home.as_ref().stack()?;

    gravity_standalone_setup(daemon_home, ADDRESS_PREFIX.as_str())
        .await
        .stack()?;
    let options = CosmovisorOptions::default();
    //options.
    let mut cosmovisor_runner = cosmovisor_start(&format!("gravity_runner_0.log"), Some(options))
        .await
        .stack()?;

    info!("successfully started");
    //sleep(TIMEOUT).await;

    sleep(Duration::ZERO).await;
    cosmovisor_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
