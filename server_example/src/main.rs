use actix_web::body::Body;
use actix_web::{middleware, App, HttpServer};
use actix_web::{web, HttpResponse, Responder};
use askama::Template;
use chrono::{DateTime, Utc};
use env_logger::Env;
use fehler::throws;
use jobclerk_server::api::handle_request;
use jobclerk_server::{make_pool, Pool, DEFAULT_POSTGRES_PORT};
use log::error;

#[derive(Template)]
#[template(path = "internal_error.html")]
struct InternalErrorTemplate {}

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsTemplate {
    projects: Vec<String>,
}

#[derive(Default)]
struct JobSummary {
    job_id: i64,
    duration: String,
    data: serde_json::Value,
    runner: String,
    state: String,
}

#[derive(Template)]
#[template(path = "project.html")]
struct ProjectTemplate {
    name: String,
    recent_jobs: Vec<JobSummary>,
    pending_jobs: Vec<JobSummary>,
    running_jobs: Vec<JobSummary>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("db error: {0}")]
    Db(#[from] tokio_postgres::Error),
    #[error("pool error: {0}")]
    Pool(#[from] bb8::RunError<tokio_postgres::Error>),
    #[error("template error: {0}")]
    Template(#[from] askama::Error),
}

impl actix_web::ResponseError for Error {
    fn error_response(&self) -> HttpResponse<Body> {
        error!("internal error: {}", self);
        let template = InternalErrorTemplate {};
        let body = match template.render() {
            Ok(body) => body,
            Err(err) => {
                error!("template error: {}", err);
                "error: failed to render error!".into()
            }
        };
        HttpResponse::InternalServerError().body(body)
    }
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

fn format_duration(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let duration = if let Ok(duration) = (end - start).to_std() {
        // Round trip the number of seconds to clear out the subsecond
        // fields
        std::time::Duration::from_secs(duration.as_secs())
    } else {
        error!("invalid duration: start={}, end={}", start, end);
        std::time::Duration::default()
    };
    humantime::format_duration(duration).to_string()
}

#[throws]
async fn get_project(
    pool: web::Data<Pool>,
    path: web::Path<(String,)>,
) -> impl Responder {
    let project_name = &path.0;
    let conn = pool.get().await?;

    let rows = conn
        .query(
            "SELECT id, data
             FROM jobs WHERE state = 'available'
             ORDER BY priority, created
             LIMIT 10",
            &[],
        )
        .await?;
    let pending_jobs = rows
        .iter()
        .map(|row| JobSummary {
            job_id: row.get(0),
            data: row.get(1),
            ..JobSummary::default()
        })
        .collect();

    let rows = conn
        .query(
            "SELECT id, data, runner, started, CURRENT_TIMESTAMP
             FROM jobs WHERE state = 'running'
             ORDER BY priority, created
             LIMIT 10",
            &[],
        )
        .await?;
    let running_jobs = rows
        .iter()
        .map(|row| {
            let started: DateTime<Utc> = row.get(3);
            let now: DateTime<Utc> = row.get(4);
            JobSummary {
                job_id: row.get(0),
                data: row.get(1),
                runner: row.get(2),
                duration: format_duration(started, now),
                ..JobSummary::default()
            }
        })
        .collect();

    let rows = conn
        .query(
            "SELECT id, data, runner, started, finished, state
             FROM jobs WHERE state != 'available' AND state != 'running'
             ORDER BY priority, created
             LIMIT 10",
            &[],
        )
        .await?;
    let recent_jobs = rows
        .iter()
        .map(|row| {
            let started: DateTime<Utc> = row.get(3);
            let now: DateTime<Utc> = row.get(4);
            JobSummary {
                job_id: row.get(0),
                data: row.get(1),
                runner: row.get(2),
                duration: format_duration(started, now),
                state: row.get(5),
            }
        })
        .collect();

    let template = ProjectTemplate {
        name: project_name.into(),
        pending_jobs,
        running_jobs,
        recent_jobs,
    };
    HttpResponse::Ok().body(template.render()?)
}

async fn handle_api_request(
    pool: web::Data<Pool>,
    req: web::Json<jobclerk_types::Request>,
) -> impl Responder {
    HttpResponse::Ok().json(handle_request(pool.get_ref(), &req).await)
}

pub fn app_config(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("")
            .route("/projects", web::get().to(list_projects))
            .route("/projects/{project_name}", web::get().to(get_project))
            .route("/api", web::post().to(handle_api_request)),
    );
}

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
