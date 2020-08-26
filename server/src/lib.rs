use actix_web::{web, HttpResponse, Responder};
use askama::Template;
use bb8_postgres::PostgresConnectionManager;
use chrono::{DateTime, Utc};
use fehler::throws;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, EnumString};
use tokio_postgres::types::ToSql;
use tokio_postgres::NoTls;

type Pool = bb8::Pool<PostgresConnectionManager<NoTls>>;
type JobId = i64;
type JobToken = String;
type ProjectId = i64;

pub const DEFAULT_POSTGRES_PORT: u16 = 5432;

fn make_random_string(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
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

#[derive(Debug, Deserialize, Serialize)]
pub struct AddProjectRequest {
    pub name: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AddProjectResponse {
    pub project_id: ProjectId,
}

#[throws]
async fn api_add_project(
    pool: web::Data<Pool>,
    data: web::Json<AddProjectRequest>,
) -> impl Responder {
    let conn = pool.get().await?;
    let row = conn
        .query_one(
            "INSERT INTO projects (name, data)
             VALUES ($1, $2)
             RETURNING id",
            &[&data.name, &data.data],
        )
        .await?;

    HttpResponse::Ok().json(AddProjectResponse {
        project_id: row.get(0),
    })
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize, AsRefStr, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum JobState {
    Available,
    Running,
    Canceling,
    Canceled,
    Succeeded,
    Failed,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Job {
    pub id: JobId,
    pub project_name: String,
    pub project_id: ProjectId,
    pub state: JobState,
    pub created: DateTime<Utc>,
    pub started: Option<DateTime<Utc>>,
    pub finished: Option<DateTime<Utc>>,
    pub priority: i32,
    pub data: serde_json::Value,
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

#[throws]
async fn api_get_job(
    pool: web::Data<Pool>,
    path: web::Path<(String, JobId)>,
) -> impl Responder {
    let project_name = &path.0;
    let job_id = &path.1;

    let conn = pool.get().await?;
    let rows = conn
        .query(
            "SELECT id, project, state, created, started, finished, priority, data
             FROM jobs
             WHERE project = (SELECT id FROM projects WHERE name = $1)
               AND id = $2",
            &[project_name, job_id],
        )
        .await?;

    if rows.is_empty() {
        HttpResponse::NotFound().finish()
    } else {
        let row = &rows[0];
        let state: String = row.get(2);
        HttpResponse::Ok().json(Job {
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
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AddJobResponse {
    pub job_id: JobId,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct TakeJobRequest {
    pub runner: String,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TakeJobResponse {
    pub job_id: Option<JobId>,
    pub job_token: Option<JobToken>,
}

/// Take ownership of an available job.
///
/// This gets the highest priority job with the oldest creation that
/// is available for this project and marks it as running. The job's
/// runner is set to the input runner, and a unique token is generated
/// so that the runner can send updates. (Updates that do not include
/// the correct token are rejected.)
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
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PatchJobRequest {
    pub token: String,
    pub state: Option<JobState>,
    pub data: Option<serde_json::Value>,
}

#[throws]
async fn api_patch_job(
    pool: web::Data<Pool>,
    path: web::Path<(String, JobId)>,
    data: web::Json<PatchJobRequest>,
) -> impl Responder {
    let project_name = &path.0;
    let job_id = &path.1;

    let conn = pool.get().await?;

    let mut stmt = "UPDATE jobs\n".to_string();
    let mut inputs: Vec<&(dyn ToSql + Sync)> =
        vec![project_name, job_id, &data.token, &data.data];
    let job_state_str;

    // Coalesce is used when setting the data so that if the data in
    // the request is null, the existing value in the row is kept.
    match data.state {
        None => {
            // No state is set, so just update the heartbeat time
            stmt += "SET heartbeat = CURRENT_TIMESTAMP,
                         data = COALESCE($4, data)";
        }
        Some(JobState::Available) => {
            // The runner has given up on the job for some reason and
            // is transitioning it from running back to
            // available. Clear the token so that more updates can't
            // be sent. Clear the started time as well.
            stmt += "SET state = 'available',
                         started = null,
                         token = null,
                         data = COALESCE($4, data)";
        }
        Some(JobState::Canceled)
        | Some(JobState::Succeeded)
        | Some(JobState::Failed) => {
            // The runner is marking the job as finished. Update the
            // finished time and clear the token so that more updates
            // can't be sent.
            stmt += "SET state = $5,
                         finished = CURRENT_TIMESTAMP,
                         token = null,
                         data = COALESCE($4, data)";
            job_state_str = data.state.as_ref().unwrap().as_ref();
            inputs.push(&job_state_str);
        }
        Some(_) => {
            // TODO
            return HttpResponse::BadRequest();
        }
    }

    stmt += "WHERE id = $2 AND project = (
                 SELECT id FROM projects WHERE name = $1) AND
               state = 'running' AND token = $3
             RETURNING id";

    let rows = conn.query(stmt.as_str(), &inputs).await?;

    if rows.is_empty() {
        // TODO
        HttpResponse::NotFound()
    } else {
        // TODO
        HttpResponse::NoContent()
    }
}

pub fn app_config(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("")
            .route("/projects", web::get().to(list_projects))
            .route("/api/projects", web::post().to(api_add_project))
            .route("/api/projects/{project}/jobs", web::get().to(api_get_jobs))
            .route(
                "/api/projects/{project}/jobs/{job}",
                web::get().to(api_get_job),
            )
            .route("/api/projects/{project}/jobs", web::post().to(api_add_job))
            .route(
                "/api/projects/{project}/take-job",
                web::post().to(api_take_job),
            )
            .route(
                "/api/projects/{project}/jobs/{job_id}",
                web::patch().to(api_patch_job),
            ),
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
