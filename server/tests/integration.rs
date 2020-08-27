use anyhow::{anyhow, Error};
use chrono::{Duration, Utc};
use env_logger::Env;
use fehler::{throw, throws};
use jobclerk_server::types::*;
use jobclerk_server::{api, Pool};
use jobclerk_server::{make_pool, DEFAULT_POSTGRES_PORT};
use serde_json::json;
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

struct CheckRequest {
    pool: Pool,
    req: Request,
    expected_response: Option<Response>,
    check_error: bool,
}

impl CheckRequest {
    async fn call(&self) -> Response {
        let resp = api::handle_request(&self.pool, &self.req).await;
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

// TODO?
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

    // Create a project
    let mut check = CheckRequest {
        pool,
        req: Request::AddProject(AddProjectRequest {
            name: "testproj".into(),
            heartbeat_expiration_millis: 250, // 0.25 seconds
            data: json!({}),
        }),
        expected_response: Some(Response::AddProject(AddProjectResponse {
            project_id: 1,
        })),
        check_error: true,
    };
    check.call().await;

    // Create a job
    check.req = Request::AddJob(AddJobRequest {
        project_name: "testproj".into(),
        data: json!({
            "hello": "world",
        }),
    });
    check.expected_response =
        Some(Response::AddJob(AddJobResponse { job_id: 1 }));
    check.call().await;

    // List jobs
    check.req = Request::GetJobs(GetJobsRequest {
        project_name: "testproj".into(),
    });
    check.expected_response = None;
    let resp = check.call().await;
    let jobs = if let Response::GetJobs(jobs) = resp {
        jobs
    } else {
        panic!("invalid response type");
    };
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
    check.req = Request::TakeJob(TakeJobRequest {
        project_name: "testproj".into(),
        runner: "testrunner".into(),
    });
    let resp = check.call().await;
    let job = if let Response::TakeJob(job) = resp {
        job
    } else {
        panic!("invalid response type");
    };
    let job = job.unwrap();
    assert_eq!(job.job_id, 1);
    let token = job.job_token.clone();
    assert_eq!(token.len(), 16);

    // Verify the job can't be taken again
    check.expected_response = Some(Response::TakeJob(None));
    check.call().await;

    // Send a heartbeat update
    check.req = Request::UpdateJob(UpdateJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
        token: token.clone(),
        state: None,
        data: None,
    });
    check.expected_response = Some(Response::Empty);
    check.call().await;

    // Verify that the job's JSON data was not changed
    check.req = Request::GetJob(GetJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
    });
    check.expected_response = None;
    let resp = check.call().await;
    assert_eq!(resp, todo

    // Update the job data
    check.req = Request::UpdateJob(UpdateJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
        token: token.clone(),
        state: None,
        data: Some(json!({"hello": "test"})),
    });
    check.expected_response = Some(Response::Empty);
    check.call().await;

    // Verify that the job's JSON data was changed
    check.req = Request::GetJob(GetJobRequest {
        project_name: "testproj".into(),
        job_id: 1,
    });
    check.expected_response = None;
    let resp = check.call().await;


        let req = test::TestRequest::get()
            .uri("/api/projects/testproj/jobs/1")
            .to_request();
        let resp: Job = test::read_response_json(&mut app, req).await;
        assert_eq!(resp.data, json!({"hello": "test"}));

    //     // Mark the job as finished
    //     let req = test::TestRequest::patch()
    //         .uri("/api/projects/testproj/jobs/1")
    //         .set_json(&UpdateJobRequest {
    //             project_name: "testproj".into(),
    //             job_id: 1,
    //             token,
    //             state: Some(JobState::Succeeded),
    //             data: None,
    //         })
    //         .to_request();
    //     let resp = test::call_service(&mut app, req).await;
    //     assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    //     // Create a second job
    //     check_json_post(
    //         &mut app,
    //         "/api/projects/testproj/jobs",
    //         json!({}),
    //         AddJobResponse { job_id: 2 },
    //     )
    //     .await;

    //     // Take the job
    //     let req = test::TestRequest::post()
    //         .uri("/api/projects/testproj/take-job")
    //         .set_json(&TakeJobRequest {
    //             project_name: "testproj".into(),
    //             runner: "testrunner".into(),
    //         })
    //         .to_request();
    //     let resp: Option<TakeJobResponse> =
    //         test::read_response_json(&mut app, req).await;
    //     let resp = resp.unwrap();
    //     assert_eq!(resp.job_id, 2);
    //     let token = resp.job_token.clone();

    //     // Sleep for 0.5 seconds which should be well past the heartbeat
    //     // expiration
    //     tokio::time::delay_for(tokio::time::Duration::from_millis(500)).await;

    //     // Poke the server to check for stuck jobs immediately (rather
    //     // than waiting for the background thread to notice)
    //     let req = test::TestRequest::post()
    //         .uri("/api/handle-stuck-jobs")
    //         .to_request();
    //     let resp = test::call_service(&mut app, req).await;
    //     assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    //     // Take the job again and verify the token has changed
    //     let req = test::TestRequest::post()
    //         .uri("/api/projects/testproj/take-job")
    //         .set_json(&TakeJobRequest {
    //             project_name: "testproj".into(),
    //             runner: "testrunner".into(),
    //         })
    //         .to_request();
    //     let resp: TakeJobResponse = test::read_response_json(&mut app, req).await;
    //     assert_eq!(resp.job_id, 2);
    //     assert_ne!(resp.job_token, token);

    Ok(())
}
