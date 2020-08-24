use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use askama::Template;
use bb8_postgres::PostgresConnectionManager;
use chrono::{DateTime, Utc};
use env_logger::Env;
use fehler::throws;
use log::info;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;
use tokio_postgres::NoTls;

type Pool = bb8::Pool<PostgresConnectionManager<NoTls>>;
type JobId = i64;
type JobToken = String;
type ProjectId = i64;

fn make_random_string(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .collect()
}

#[derive(Debug, thiserror::Error)]
enum Error {
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

#[derive(Debug, Serialize, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
enum JobState {
    Available,
    Activating,
    Running,
    Canceling,
    Canceled,
    Succeeded,
    Failed,
}

#[derive(Debug, Serialize)]
struct Job {
    id: JobId,
    project_name: String,
    project_id: ProjectId,
    state: JobState,
    created: DateTime<Utc>,
    started: Option<DateTime<Utc>>,
    finished: Option<DateTime<Utc>>,
    priority: i32,
    data: serde_json::Value,
}

#[throws]
async fn api_get_jobs(
    pool: web::Data<Pool>,
    path: web::Path<(String,)>,
) -> impl Responder {
    let project_name = &path.0;

    let conn = pool.get().await?;
    let rows = conn
        .query(
            "SELECT id, project, state, created, started, finished, priority, data
             FROM jobs
             WHERE project = (SELECT id FROM projects WHERE name = $1)",
            &[project_name],
        )
        .await?;

    let jobs = rows
        .iter()
        .map(|row| -> Result<Job, Error> {
            let state: String = row.get(2);
            Ok(Job {
                id: row.get(0),
                project_name: project_name.clone(),
                project_id: row.get(1),
                state: state.parse()?,
                created: row.get(3),
                started: row.get(4),
                finished: row.get(5),
                priority: row.get(6),
                data: row.get(7),
            })
        })
        .collect::<Result<Vec<Job>, _>>()?;

    HttpResponse::Ok().json(jobs)
}

#[derive(Debug, Serialize)]
struct AddJobResponse {
    job_id: JobId,
}

#[throws]
async fn api_add_job(
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

#[derive(Debug, Deserialize)]
struct TakeJobRequest {
    runner: String,
}

#[derive(Debug, Serialize)]
struct TakeJobResponse {
    job_id: Option<JobId>,
    job_token: Option<JobToken>,
}

#[throws]
async fn api_take_job(
    pool: web::Data<Pool>,
    path: web::Path<(String,)>,
    data: web::Json<TakeJobRequest>,
) -> impl Responder {
    let project_name = &path.0;

    let token = make_random_string(16);

    let conn = pool.get().await?;
    // TODO: do we need to explictly start a transaction here?
    let rows = conn
        .query(
            include_str!("../../db/query_take_job.sql"),
            &[project_name, &data.runner, &token],
        )
        .await?;

    if rows.is_empty() {
        HttpResponse::Ok().json(TakeJobResponse {
            job_id: None,
            job_token: None,
        })
    } else {
        let row = &rows[0];
        HttpResponse::Ok().json(TakeJobResponse {
            job_id: Some(row.get(0)),
            job_token: Some(row.get(1)),
        })
    }

    // Sketch of the protocol:
    //
    // client calls /projects/blah/take-job
    //
    // server atomically finds an available job and marks it as
    // running. as part of that atomic operation a unique (random)
    // "owner" ID is set in the job's row.
    //
    // take-job returns that job ID and owner ID.
    //
    // The client can now use those IDs to update the job with a patch
    // take or similar. Note that only a client with the correct
    // owner ID can update the job, as a layer of protection against
    // clients fighting over the same job.

    // TODO
}

#[throws(anyhow::Error)]
#[actix_rt::main]
async fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    let db_manager = PostgresConnectionManager::new_from_stringlike(
        "host=localhost user=postgres",
        NoTls,
    )?;

    let pool = Pool::builder().build(db_manager).await?;

    info!("starting server on port 8000");
    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .route("/projects", web::get().to(list_projects))
            .route("/api/projects/{project}/jobs", web::get().to(api_get_jobs))
            .route("/api/projects/{project}/jobs", web::post().to(api_add_job))
            .route(
                "/api/projects/{project}/take-job",
                web::post().to(api_take_job),
            )
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await?
}
