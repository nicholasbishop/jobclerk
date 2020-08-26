use actix_web::{middleware, test, App};
use anyhow::{anyhow, Error};
use env_logger::Env;
use fehler::{throw, throws};
use jobclerk_server::{
    app_config, make_pool, AddJobResponse, AddProjectRequest,
    AddProjectResponse, DEFAULT_POSTGRES_PORT,
};
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

    let req = test::TestRequest::post()
        .uri("/api/projects")
        .set_json(&AddProjectRequest {
            name: "testproj".into(),
        })
        .to_request();
    let resp: AddProjectResponse =
        test::read_response_json(&mut app, req).await;
    assert_eq!(resp, AddProjectResponse { project_id: 1 });

    let req = test::TestRequest::post()
        .uri("/api/projects/testproj/jobs")
        .set_json(&json!({
            "hello": "world",
        }))
        .to_request();
    let resp: AddJobResponse = test::read_response_json(&mut app, req).await;
    assert_eq!(resp, AddJobResponse { job_id: 1 });

    Ok(())
}
