use test_runner::run_test;

#[tokio::main]
pub async fn main() {
    env_logger::init();
    run_test().await;
}
