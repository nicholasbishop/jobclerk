use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use askama::Template;
use bb8_postgres::PostgresConnectionManager;
use env_logger::Env;
use fehler::throws;
use log::info;
use serde::Serialize;
use tokio_postgres::NoTls;

type Pool = bb8::Pool<PostgresConnectionManager<NoTls>>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("db error: {0}")]
    Db(#[from] tokio_postgres::Error),
    #[error("pool error: {0}")]
    Pool(#[from] bb8::RunError<tokio_postgres::Error>),
    #[error("template error: {0}")]
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

type JobId = i64;

#[derive(Debug, Serialize)]
struct AddJobResponse {
    job_id: JobId,
}

#[throws]
async fn add_job(
    pool: web::Data<Pool>,
    path: web::Path<(String,)>,
    data: web::Json<serde_json::Value>,
) -> impl Responder {
    let project_name = &path.0;
    let data = data.into_inner();

    let conn = pool.get().await?;
    let row = conn
        .query_one(
            "INSERT INTO jobs (project, data)
             VALUES ((SELECT id FROM projects WHERE name = $1), $2)
             RETURNING id",
            &[project_name, &data],
        )
        .await?;

    let job_id: JobId = row.get(0);

    HttpResponse::Ok().json(AddJobResponse { job_id })
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
            .route("/projects/{project}/jobs", web::post().to(add_job))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await?
}
