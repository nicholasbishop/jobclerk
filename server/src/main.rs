use env_logger::Env;
use fehler::throws;
use jobclerk_server::run_server;

#[throws(anyhow::Error)]
#[actix_rt::main]
async fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    run_server().await?;
}
