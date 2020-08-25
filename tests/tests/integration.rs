use anyhow::{anyhow, Error};
use fehler::{throw, throws};
use std::process::{Child, Command};

const POSTGRES_CONTAINER_NAME: &str = "jobclerk-test-postgres";

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

struct ProcDrop {
    child: Child,
}

impl ProcDrop {
    fn new(child: Child) -> ProcDrop {
        ProcDrop { child }
    }
}

impl Drop for ProcDrop {
    fn drop(&mut self) {
        self.child.kill();
    }
}

fn get_postgres_cmd(action: &str) -> Command {
    let mut cmd = Command::new("docker");
    cmd.args(&[action, POSTGRES_CONTAINER_NAME]);
    cmd
}

#[throws]
fn run_postgres() {
    let pg_port = 5433;

    // Stop the container if it already exists
    run_cmd_no_check(&mut get_postgres_cmd("stop"))?;

    run_cmd(Command::new("docker").args(&[
        "run",
        "--rm",
        "--name",
        POSTGRES_CONTAINER_NAME,
        "--publish",
        &format!("{0}:{0}", pg_port),
        // Allow all connections without a password. This is just a
        // test database so it's fine.
        "-e",
        "POSTGRES_HOST_AUTH_METHOD=trust",
        "-d",
        "postgres:alpine",
    ]))?;
}

#[throws]
#[test]
fn integration_test() {
    // Run the database
    run_postgres()?;
    let _stop_postgres = RunOnDrop::new(get_postgres_cmd("kill"));

    // Run the server
    let _server =
        ProcDrop::new(Command::new("../target/debug/jobclerk-server").spawn()?);

    // TODO: make requests and test the result
}
