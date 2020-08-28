use argh::FromArgs;
use jobclerk_types::*;

/// Create a job within a project.
#[derive(FromArgs)]
#[argh(subcommand, name = "add-job")]
struct AddJob {
    #[argh(positional)]
    project_name: String,

    #[argh(positional)]
    data: serde_json::Value,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    AddJob(AddJob),
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

    let req = match opt.command {
        Command::AddJob(opt) => Request::AddJob(AddJobRequest {
            project_name: opt.project_name,
            data: opt.data,
        }),
    };

    let resp = ureq::post(&url).send_json(
        serde_json::to_value(req).expect("failed to convert request to JSON"),
    );
    println!("{}", resp.into_json().expect("response is not json"));
}
