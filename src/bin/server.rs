use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use askama::Template;
use bb8_postgres::PostgresConnectionManager;
use env_logger::Env;
use fehler::throws;
use log::info;
use tokio_postgres::NoTls;

type Pool = bb8::Pool<PostgresConnectionManager<NoTls>>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("db error")]
    Db(#[from] tokio_postgres::Error),
    #[error("pool error")]
    Pool(#[from] bb8::RunError<tokio_postgres::Error>),
    #[error("template error")]
    Template(#[from] askama::Error),
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

#[throws(anyhow::Error)]
#[actix_rt::main]
async fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    let db_manager =
        PostgresConnectionManager::new_from_stringlike("host=localhost user=postgres", NoTls)?;

    let pool = Pool::builder().build(db_manager).await?;

    info!("starting server on port 8000");
    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .route("/projects", web::get().to(list_projects))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await?
}
