use actix_http::Request;
use actix_web::dev::{MessageBody, Service, ServiceResponse};
use actix_web::http::StatusCode;
use actix_web::{middleware, test, App};
use anyhow::{anyhow, Error};
use chrono::{Duration, Utc};
use env_logger::Env;
use fehler::{throw, throws};
use jobclerk_server::{
    app_config, make_pool, AddJobResponse, AddProjectRequest,
    AddProjectResponse, Job, JobState, PatchJobRequest, TakeJobRequest,
    TakeJobResponse, DEFAULT_POSTGRES_PORT,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use std::fmt;
use std::process::Command;

const POSTGRES_CONTAINER_NAME: &str = "jobclerk-test-postgres";
const POSTGRES_PORT: u16 = 5433;

fn cmd_str(cmd: &Command) -> String {
    format!("{:?}", cmd).replace('"', "")
}

#[throws]
fn run_cmd(cmd: &mut Command) {
    let cmd_str = cmd_str(cmd);
    println!("{}", cmd_str);
    let status = cmd.status()?;
    if !status.success() {
        throw!(anyhow!("command {} failed: {}", cmd_str, status));
    }
}

#[throws]
fn run_cmd_no_check(cmd: &mut Command) {
    let cmd_str = format!("{:?}", cmd).replace('"', "");
    println!("{}", cmd_str);
    cmd.status()?;
}

struct RunOnDrop {
    cmd: Command,
}

impl RunOnDrop {
    fn new(cmd: Command) -> RunOnDrop {
        RunOnDrop { cmd }
    }
}

impl Drop for RunOnDrop {
    fn drop(&mut self) {
        if let Err(err) = run_cmd_no_check(&mut self.cmd) {
            let cmd_str = cmd_str(&self.cmd);
            eprintln!("failed to run '{}': {}", cmd_str, err);
        }
    }
}

fn get_postgres_cmd(action: &str) -> Command {
    let mut cmd = Command::new("docker");
    cmd.args(&[action, POSTGRES_CONTAINER_NAME]);
    cmd
}

#[throws]
fn run_postgres() {
    // Stop the container if it already exists
    run_cmd_no_check(&mut get_postgres_cmd("stop"))?;

    run_cmd(Command::new("docker").args(&[
        "run",
        "--rm",
        "--name",
        POSTGRES_CONTAINER_NAME,
        "--publish",
        &format!("{}:{}", POSTGRES_PORT, DEFAULT_POSTGRES_PORT),
        // Allow all connections without a password. This is just a
        // test database so it's fine.
        "-e",
        "POSTGRES_HOST_AUTH_METHOD=trust",
        "-d",
        "postgres:alpine",
    ]))?;
}

async fn check_json_post<'a, B, App, S, D>(
    app: &mut App,
    url: &str,
    body: S,
    expected: D,
) where
    App: Service<
        Request = Request,
        Response = ServiceResponse<B>,
        Error = actix_web::Error,
    >,
    B: MessageBody,
    S: Serialize,
    D: DeserializeOwned + fmt::Debug + Eq,
{
    let req = test::TestRequest::post()
        .uri(url)
        .set_json(&body)
        .to_request();
    let resp: D = test::read_response_json(app, req).await;
    assert_eq!(resp, expected);
}

#[actix_rt::test]
async fn integration_test() -> Result<(), Error> {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    // Run and initialize the database
    run_postgres()?;
    let _stop_postgres = RunOnDrop::new(get_postgres_cmd("kill"));
    let pool = make_pool(POSTGRES_PORT).await?;
    {
        let conn = pool.get().await?;
        conn.batch_execute(include_str!("../../db/init.sql"))
            .await?;
    }

    // Run the server
    let mut app = test::init_service(
        App::new()
            .wrap(middleware::Logger::default())
            .configure(app_config)
            .data(pool),
    )
    .await;

    // Create a project
    check_json_post(
        &mut app,
        "/api/projects",
        AddProjectRequest {
            name: "testproj".into(),
            data: json!({}),
        },
        AddProjectResponse { project_id: 1 },
    )
    .await;

    // Create a job
    check_json_post(
        &mut app,
        "/api/projects/testproj/jobs",
        json!({
            "hello": "world",
        }),
        AddJobResponse { job_id: 1 },
    )
    .await;

    // List jobs
    let req = test::TestRequest::get()
        .uri("/api/projects/testproj/jobs")
        .to_request();
    let resp: Vec<Job> = test::read_response_json(&mut app, req).await;
    assert_eq!(resp.len(), 1);
    let job = &resp[0];
    // Check the created time separately since there's wiggle room
    assert!(
        Utc::now().signed_duration_since(job.created) < Duration::seconds(1)
    );
    assert_eq!(
        job,
        &Job {
            id: 1,
            project_id: 1,
            project_name: "testproj".into(),
            state: JobState::Available,
            created: job.created,
            started: None,
            finished: None,
            priority: 0,
            data: json!({
                "hello": "world",
            })
        }
    );

    // Take a job
    let req = test::TestRequest::post()
        .uri("/api/projects/testproj/take-job")
        .set_json(&TakeJobRequest {
            runner: "testrunner".into(),
        })
        .to_request();
    let resp: TakeJobResponse = test::read_response_json(&mut app, req).await;
    assert_eq!(resp.job_id, Some(1));
    let token = resp.job_token.clone().unwrap();
    assert_eq!(token.len(), 16);

    // Verify the job can't be taken again
    check_json_post(
        &mut app,
        "/api/projects/testproj/take-job",
        TakeJobRequest {
            runner: "testrunner".into(),
        },
        TakeJobResponse {
            job_id: None,
            job_token: None,
        },
    )
    .await;

    // Send a heartbeat update
    let req = test::TestRequest::patch()
        .uri("/api/projects/testproj/jobs/1")
        .set_json(&PatchJobRequest {
            token,
            state: None,
            data: None,
        })
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    Ok(())
}
