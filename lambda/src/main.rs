use env_logger::Env;
use jobclerk_server::api::handle_request;
use jobclerk_server::{make_pool, Pool, DEFAULT_POSTGRES_PORT};
use jobclerk_types::{Request, Response};
use lambda::{handler_fn, Context};
use once_cell::sync::OnceCell;
use std::convert::Infallible;

// Keep the pool in a OnceCell so that we know it's only initialized
// once.
static POOL: OnceCell<Pool> = OnceCell::new();

async fn lambda_handler(
    req: Request,
    _: Context,
) -> Result<Response, Infallible> {
    let pool = POOL.get().expect("pool is not initialized");
    Ok(handle_request(pool, &req).await)
}

#[tokio::main]
async fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    // TODO: need to add host and such to the params here
    POOL.set(
        make_pool(DEFAULT_POSTGRES_PORT)
            .await
            .expect("failed to initialize pool"),
    )
    .expect("pool is already initialized");

    let func = handler_fn(lambda_handler);
    lambda::run(func).await.expect("failed to run lambda");
}
