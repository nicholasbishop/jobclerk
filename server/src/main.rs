use actix_web::{middleware, App, HttpServer};
use env_logger::Env;
use fehler::throws;
use jobclerk_server::{app_config, make_pool, DEFAULT_POSTGRES_PORT};

#[throws(anyhow::Error)]
#[actix_rt::main]
async fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    let pool = make_pool(DEFAULT_POSTGRES_PORT).await?;

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .configure(app_config)
            .data(pool.clone())
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await?;
}
