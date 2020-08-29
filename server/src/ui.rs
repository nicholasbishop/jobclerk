use crate::{Error, Pool};
use askama::Template;
use chrono::{DateTime, Utc};
use fehler::throws;
use log::error;

#[derive(Template)]
#[template(path = "internal_error.html")]
struct InternalErrorTemplate {}

pub fn internal_error() -> String {
    let template = InternalErrorTemplate {};
    match template.render() {
        Ok(body) => body,
        Err(err) => {
            error!("template error: {}", err);
            "error: failed to render error!".into()
        }
    }
}

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsTemplate {
    projects: Vec<String>,
}

#[throws]
pub async fn list_projects(pool: &Pool) -> String {
    let conn = pool.get().await?;
    let rows = conn.query("SELECT id, name FROM projects", &[]).await?;

    let template = ProjectsTemplate {
        projects: rows.iter().map(|row| row.get(1)).collect(),
    };
    template.render()?
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
pub async fn get_project(pool: &Pool, project_name: &str) -> String {
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
    template.render()?
}
