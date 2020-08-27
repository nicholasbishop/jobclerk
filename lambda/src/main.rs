use env_logger::Env;
use jobclerk_api::{handle_request, make_pool, DEFAULT_POSTGRES_PORT};
use jobclerk_types::{Request, Response};
use lambda::{lambda, Context};
use std::convert::Infallible;

#[lambda]
#[tokio::main]
async fn main(req: Request, _: Context) -> Result<Response, Infallible> {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    // TODO: need to add host and such to the params here
    // TODO: does this need to be done outside main to share between requests?
    let pool = make_pool(DEFAULT_POSTGRES_PORT).await.unwrap();

    Ok(handle_request(&pool, &req).await)
}
