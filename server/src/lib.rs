pub mod api;

use actix_web::{web, HttpResponse, Responder};
use askama::Template;
use bb8_postgres::PostgresConnectionManager;
use fehler::throws;
use tokio_postgres::NoTls;

pub type Pool = bb8::Pool<PostgresConnectionManager<NoTls>>;

pub const DEFAULT_POSTGRES_PORT: u16 = 5432;

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
    #[error("template error: {0}")]
    Template(#[from] askama::Error),
    #[error("parse error: {0}")]
    Parse(#[from] strum::ParseError),
}

impl actix_web::error::ResponseError for Error {}

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsTemplate {
    projects: Vec<String>,
}

#[throws]
async fn list_projects(pool: web::Data<Pool>) -> impl Responder {
    let conn = pool.get().await?;
    let rows = conn.query("SELECT id, name FROM projects", &[]).await?;

    let template = ProjectsTemplate {
        projects: rows.iter().map(|row| row.get(1)).collect(),
    };
    HttpResponse::Ok().body(template.render()?)
}

async fn handle_api_request(
    pool: web::Data<Pool>,
    req: web::Json<jobclerk_types::Request>,
) -> impl Responder {
    HttpResponse::Ok().json(api::handle_request(pool.get_ref(), &req).await)
}

pub fn app_config(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("")
            .route("/projects", web::post().to(list_projects))
            .route("/api", web::post().to(handle_api_request)),
    );
}

#[throws(anyhow::Error)]
pub async fn make_pool(port: u16) -> Pool {
    let db_manager = PostgresConnectionManager::new_from_stringlike(
        format!("host=localhost user=postgres port={}", port),
        NoTls,
    )?;

    Pool::builder().build(db_manager).await?
}
