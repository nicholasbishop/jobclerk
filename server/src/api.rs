use crate::types::*;
use crate::{Error, Pool};
use fehler::{throw, throws};
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use tokio_postgres::types::ToSql;

fn make_random_string(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .collect()
}

#[throws]
async fn add_project(
    pool: &Pool,
    req: &AddProjectRequest,
) -> AddProjectResponse {
    if req.heartbeat_expiration_millis <= 0 {
        throw!(Error::BadRequest(format!(
            "invalid heartbeat_expiration_millis: {}",
            req.heartbeat_expiration_millis
        ),));
    }

    let conn = pool.get().await?;
    let row = conn
        .query_one(
            "INSERT INTO projects (name, heartbeat_expiration_millis, data)
             VALUES ($1, $2, $3)
             RETURNING id",
            &[&req.name, &req.heartbeat_expiration_millis, &req.data],
        )
        .await?;

    AddProjectResponse {
        project_id: row.get(0),
    }
}

#[throws]
async fn get_job(pool: &Pool, req: &GetJobRequest) -> Job {
    let conn = pool.get().await?;
    let rows = conn
        .query(
            "SELECT id, project, state, created, started, finished, priority, data
             FROM jobs
             WHERE project = (SELECT id FROM projects WHERE name = $1)
               AND id = $2",
            &[&req.project_name, &req.job_id],
        )
        .await?;

    if rows.is_empty() {
        throw!(Error::NotFound);
    } else {
        let row = &rows[0];
        let state: String = row.get(2);
        Job {
            id: row.get(0),
            project_name: req.project_name.clone(),
            project_id: row.get(1),
            state: state.parse()?,
            created: row.get(3),
            started: row.get(4),
            finished: row.get(5),
            priority: row.get(6),
            data: row.get(7),
        }
    }
}

#[throws]
async fn get_jobs(pool: &Pool, req: &GetJobsRequest) -> Vec<Job> {
    let conn = pool.get().await?;
    let rows = conn
        .query(
            "SELECT id, project, state, created, started, finished, priority, data
             FROM jobs
             WHERE project = (SELECT id FROM projects WHERE name = $1)",
            &[&req.project_name],
        )
        .await?;

    let jobs = rows
        .iter()
        .map(|row| -> Result<Job, Error> {
            let state: String = row.get(2);
            Ok(Job {
                id: row.get(0),
                project_name: req.project_name.clone(),
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

    jobs
}

#[throws]
async fn add_job(pool: &Pool, req: &AddJobRequest) -> AddJobResponse {
    let conn = pool.get().await?;
    let row = conn
        .query_one(
            "INSERT INTO jobs (project, data)
             VALUES ((SELECT id FROM projects WHERE name = $1), $2)
             RETURNING id",
            &[&req.project_name, &req.data],
        )
        .await?;

    let job_id: JobId = row.get(0);

    AddJobResponse { job_id }
}

/// Take ownership of an available job.
///
/// This gets the highest priority job with the oldest creation that
/// is available for this project and marks it as running. The job's
/// runner is set to the input runner, and a unique token is generated
/// so that the runner can send updates. (Updates that do not include
/// the correct token are rejected.)
#[throws]
async fn take_job(
    pool: &Pool,
    req: &TakeJobRequest,
) -> Option<TakeJobResponse> {
    let token = make_random_string(16);

    let conn = pool.get().await?;
    // TODO: do we need to explictly start a transaction here?
    let rows = conn
        .query(
            include_str!("../../db/query_take_job.sql"),
            &[&req.project_name, &req.runner, &token],
        )
        .await?;

    if rows.is_empty() {
        None
    } else {
        let row = &rows[0];
        Some(TakeJobResponse {
            job_id: row.get(0),
            job_token: row.get(1),
        })
    }
}

#[throws]
async fn handle_stuck_jobs(pool: &Pool) {
    let conn = pool.get().await?;
    conn.query(include_str!("../../db/query_handle_stuck_jobs.sql"), &[])
        .await?;
}

#[throws]
async fn update_job(pool: &Pool, req: &UpdateJobRequest) {
    let conn = pool.get().await?;

    let mut stmt = "UPDATE jobs\n".to_string();
    let mut inputs: Vec<&(dyn ToSql + Sync)> =
        vec![&req.project_name, &req.job_id, &req.token, &req.data];
    let job_state_str;

    // Coalesce is used when setting the data so that if the data in
    // the request is null, the existing value in the row is kept.
    match &req.state {
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
            job_state_str = req.state.as_ref().unwrap().as_ref();
            inputs.push(&job_state_str);
        }
        Some(state) => {
            throw!(Error::BadRequest(format!(
                "invalid state: {}",
                state.as_ref()
            )));
        }
    }

    stmt += "WHERE id = $2 AND project = (
                 SELECT id FROM projects WHERE name = $1) AND
               state = 'running' AND token = $3
             RETURNING id";

    let rows = conn.query(stmt.as_str(), &inputs).await?;

    if rows.is_empty() {
        throw!(Error::NotFound)
    }
}

#[throws]
async fn handle_request_ok(pool: &Pool, req: &Request) -> Response {
    match req {
        Request::AddProject(req) => {
            Response::AddProject(add_project(pool, req).await?)
        }

        Request::AddJob(req) => Response::AddJob(add_job(pool, req).await?),
        Request::GetJob(req) => Response::GetJob(get_job(pool, req).await?),
        Request::GetJobs(req) => Response::GetJobs(get_jobs(pool, req).await?),
        Request::TakeJob(req) => Response::TakeJob(take_job(pool, req).await?),
        Request::UpdateJob(req) => {
            update_job(pool, req).await?;
            Response::Empty
        }
        Request::HandleStuckJobs => {
            handle_stuck_jobs(pool).await?;
            Response::Empty
        }
    }
}

fn handle_request_err(err: Error) -> Response {
    match err {
        Error::BadRequest(s) => Response::BadRequest(s),
        Error::NotFound => Response::NotFound,
        Error::Db(_) => Response::InternalError,
        Error::Pool(_) => Response::InternalError,
        Error::Template(_) => Response::InternalError,
        Error::Parse(_) => Response::InternalError,
    }
}

pub async fn handle_request(pool: &Pool, req: &Request) -> Response {
    info!("request: {:?}", req);
    match handle_request_ok(pool, req).await {
        Ok(resp) => resp,
        Err(err) => {
            error!("error: {}", err);
            handle_request_err(err)
        }
    }
}
