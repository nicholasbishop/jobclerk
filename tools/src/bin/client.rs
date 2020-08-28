use argh::FromArgs;
use jobclerk_types::*;

/// Create a project.
#[derive(FromArgs)]
#[argh(subcommand, name = "add-project")]
struct AddProject {
    #[argh(positional)]
    name: String,

    /// length of time in seconds before jobs are considered stuck
    #[argh(option, default = "30")]
    grace_period: i32,

    /// set the project data
    #[argh(option, default = "serde_json::json!({})")]
    data: serde_json::Value,
}

/// Create a job within a project.
#[derive(FromArgs)]
#[argh(subcommand, name = "add-job")]
struct AddJob {
    #[argh(positional)]
    project_name: String,

    #[argh(positional)]
    data: serde_json::Value,
}

/// Start running an available job.
#[derive(FromArgs)]
#[argh(subcommand, name = "take-job")]
struct TakeJob {
    #[argh(positional)]
    project_name: String,

    #[argh(positional)]
    runner: String,
}

/// Update a running job.
#[derive(FromArgs)]
#[argh(subcommand, name = "update-job")]
struct UpdateJob {
    #[argh(positional)]
    project_name: String,

    #[argh(positional)]
    job_id: JobId,

    #[argh(positional)]
    token: JobToken,

    /// set the job state
    #[argh(option)]
    state: Option<JobState>,

    /// set the job data
    #[argh(option)]
    data: Option<serde_json::Value>,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    AddProject(AddProject),

    AddJob(AddJob),
    TakeJob(TakeJob),
    UpdateJob(UpdateJob),
}

/// Send a request to the server and print the response.
#[derive(FromArgs)]
struct Opt {
    /// base URL of the server (including scheme)
    #[argh(option, default = "\"http://localhost:8000\".into()")]
    base_url: String,

    #[argh(subcommand)]
    command: Command,
}

fn main() {
    let opt: Opt = argh::from_env();
    let url = format!("{}/api", opt.base_url);

    let req: Request = match opt.command {
        Command::AddProject(opt) => AddProjectRequest {
            name: opt.name,
            data: opt.data,
            heartbeat_expiration_millis: opt.grace_period * 1000,
        }
        .into(),
        Command::AddJob(opt) => AddJobRequest {
            project_name: opt.project_name,
            data: opt.data,
        }
        .into(),
        Command::TakeJob(opt) => TakeJobRequest {
            project_name: opt.project_name,
            runner: opt.runner,
        }
        .into(),
        Command::UpdateJob(opt) => UpdateJobRequest {
            project_name: opt.project_name,
            job_id: opt.job_id,
            state: opt.state,
            data: opt.data,
            token: opt.token,
        }
        .into(),
    };

    let resp = ureq::post(&url).send_json(
        serde_json::to_value(req).expect("failed to convert request to JSON"),
    );
    println!("{}", resp.into_json().expect("response is not json"));
}
