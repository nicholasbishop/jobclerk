pub mod api;
pub mod ui;

use bb8_postgres::PostgresConnectionManager;
use fehler::throws;
use tokio_postgres::NoTls;

pub type Pool = bb8::Pool<PostgresConnectionManager<NoTls>>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("db error: {0}")]
    Db(#[from] tokio_postgres::Error),
    #[error("pool error: {0}")]
    Pool(#[from] bb8::RunError<tokio_postgres::Error>),
    #[error("parse error: {0}")]
    Parse(#[from] strum::ParseError),
    #[error("template error: {0}")]
    Template(#[from] askama::Error),
}

pub const DEFAULT_POSTGRES_PORT: u16 = 5432;

#[throws]
pub async fn make_pool(port: u16) -> Pool {
    let db_manager = PostgresConnectionManager::new_from_stringlike(
        format!("host=localhost user=postgres port={}", port),
        NoTls,
    )?;

    Pool::builder().build(db_manager).await?
}
