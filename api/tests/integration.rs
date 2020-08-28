use chrono::{Duration, Utc};
use env_logger::Env;
use jobclerk_api::{handle_request, make_pool, Pool};
use jobclerk_types::*;
use serde_json::json;
use std::process::Command;

const POSTGRES_CONTAINER_NAME: &str = "jobclerk-test-postgres";
const POSTGRES_PORT: u16 = 5433;

fn cmd_str(cmd: &Command) -> String {
    format!("{:?}", cmd).replace('"', "")
}

fn run_cmd(cmd: &mut Command) {
    let cmd_str = cmd_str(cmd);
    println!("{}", cmd_str);
    let status = cmd.status().unwrap();
    if !status.success() {
        panic!("command {} failed: {}", cmd_str, status);
    }
}

fn run_cmd_no_check(cmd: &mut Command) {
    let cmd_str = format!("{:?}", cmd).replace('"', "");
    println!("{}", cmd_str);
    cmd.status().unwrap();
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
        run_cmd_no_check(&mut self.cmd);
    }
}

fn get_postgres_cmd(action: &str) -> Command {
    let mut cmd = Command::new("docker");
    cmd.args(&[action, POSTGRES_CONTAINER_NAME]);
    cmd
}

fn run_postgres() {
    // Stop the container if it already exists
    run_cmd_no_check(&mut get_postgres_cmd("stop"));

    run_cmd(Command::new("docker").args(&[
        "run",
        "--rm",
        "--name",
        POSTGRES_CONTAINER_NAME,
        "--publish",
        &format!("{}:{}", POSTGRES_PORT, 5432),
        // Allow all connections without a password. This is just a
        // test database so it's fine.
        "-e",
        "POSTGRES_HOST_AUTH_METHOD=trust",
        "-d",
        "postgres:alpine",
    ]));
}

struct CheckRequest {
    pool: Pool,
    req: Request,
    expected_response: Option<Response>,
    check_error: bool,
}

impl CheckRequest {
    async fn call(&self) -> Response {
        let resp = handle_request(&self.pool, &self.req).await;
        if let Some(expected_response) = &self.expected_response {
            assert_eq!(&resp, expected_response);
        } else if self.check_error {
            if resp.is_error() {
                panic!("call failed with: {:?}", resp);
            }
        }
        resp
    }
}

#[tokio::test]
async fn integration_test() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    // Run and initialize the database
    run_postgres();
    let _stop_postgres = RunOnDrop::new(get_postgres_cmd("kill"));
    let pool = make_pool(POSTGRES_PORT).await.unwrap();
    {
        let conn = pool.get().await.unwrap();
        conn.batch_execute(include_str!("../../db/init.sql"))
            .await
            .unwrap();
    }

    // Create a project
    let mut check = CheckRequest {
        pool,
        req: AddProjectRequest {
            name: "testproj".into(),
            heartbeat_expiration_millis: 250, // 0.25 seconds
            data: json!({}),
        }
        .into(),
        expected_response: Some(Response::AddProject(AddProjectResponse {
            project_id: 1,
        })),
        check_error: true,
    };
    check.call().await;

    // Create a job
    check.req = AddJobRequest {
        project_name: "testproj".into(),
        data: json!({
            "hello": "world",
        }),
    }
    .into();
    check.expected_response =
        Some(Response::AddJob(AddJobResponse { job_id: 1 }));
    check.call().await;

    // List jobs
    check.req = GetJobsRequest {
        project_name: "testproj".into(),
    }
    .into();
    check.expected_response = None;
    let jobs = check.call().await.into_get_jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    let job = &jobs[0];
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
    check.req = TakeJobRequest {
        project_name: "testproj".into(),
        runner: "testrunner".into(),
    }
    .into();
    let job = check.call().await.into_take_job().unwrap().unwrap();
    assert_eq!(job.job_id, 1);
    let token = job.job_token.clone();
    assert_eq!(token.len(), 16);

    // Verify the job can't be taken again
    check.expected_response = Some(Response::TakeJob(None));
    check.call().await;

    // Send a heartbeat update
    check.req = UpdateJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
        token: token.clone(),
        state: None,
        data: None,
    }
    .into();
    check.expected_response = Some(Response::Empty);
    check.call().await;

    // Verify that the job's JSON data was not changed
    check.req = GetJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
    }
    .into();
    check.expected_response = None;
    let resp = check.call().await.into_get_job().unwrap();
    assert_eq!(resp.data, json!({"hello": "world"}));

    // Update the job data
    check.req = UpdateJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
        token: token.clone(),
        state: None,
        data: Some(json!({"hello": "test"})),
    }
    .into();
    check.expected_response = Some(Response::Empty);
    check.call().await;

    // Verify that the job's JSON data was changed
    check.req = GetJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
    }
    .into();
    check.expected_response = None;
    let resp = check.call().await.into_get_job().unwrap();
    assert_eq!(resp.data, json!({"hello": "test"}));

    // Mark the job as finished
    check.req = UpdateJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
        token,
        state: Some(JobState::Succeeded),
        data: None,
    }
    .into();
    check.expected_response = Some(Response::Empty);
    check.call().await;

    // Create a second job
    check.req = AddJobRequest {
        project_name: "testproj".into(),
        data: json!({}),
    }
    .into();
    check.expected_response =
        Some(Response::AddJob(AddJobResponse { job_id: 2 }));
    check.call().await;

    // Take the job
    check.req = TakeJobRequest {
        project_name: "testproj".into(),
        runner: "testrunner".into(),
    }
    .into();
    check.expected_response = None;
    let job = check.call().await.into_take_job().unwrap().unwrap();
    assert_eq!(job.job_id, 2);
    let token = job.job_token.clone();

    // Sleep for 0.5 seconds which should be well past the heartbeat
    // expiration
    tokio::time::delay_for(tokio::time::Duration::from_millis(500)).await;

    // Poke the server to check for stuck jobs
    check.req = Request::HandleStuckJobs;
    check.expected_response = Some(Response::Empty);
    check.call().await;

    // Take the job again and verify the token has changed
    check.req = TakeJobRequest {
        project_name: "testproj".into(),
        runner: "testrunner".into(),
    }
    .into();
    check.expected_response = None;
    let job = check.call().await.into_take_job().unwrap().unwrap();
    assert_eq!(job.job_id, 2);
    assert_ne!(job.job_token, token);
}
