use chrono::{DateTime, Utc};
use paste::paste;
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, EnumString};

pub type JobId = i64;
pub type JobToken = String;
pub type ProjectId = i64;

macro_rules! request_from {
    ($name:ident) => {
        paste! {
            impl From<[<$name Request>]> for Request {
                fn from(request: [<$name Request>]) -> Request {
                    Request::$name(request)
                }
            }
        }
    };
}

macro_rules! response_from {
    ($name:ident) => {
        paste! {
            impl From<[<$name Response>]> for Response {
                fn from(request: [<$name Response>]) -> Response {
                    Response::$name(request)
                }
            }
        }
    };
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Request {
    AddProject(AddProjectRequest),

    AddJob(AddJobRequest),
    GetJob(GetJobRequest),
    GetJobs(GetJobsRequest),
    TakeJob(TakeJobRequest),
    UpdateJob(UpdateJobRequest),

    HandleStuckJobs,
}

request_from!(AddProject);
request_from!(AddJob);
request_from!(GetJob);
request_from!(GetJobs);
request_from!(TakeJob);
request_from!(UpdateJob);

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Response {
    AddProject(AddProjectResponse),
    AddJob(AddJobResponse),
    GetJob(GetJobResponse),
    GetJobs(GetJobsResponse),
    TakeJob(TakeJobResponse),
    Empty,

    BadRequest(String),
    NotFound,
    InternalError,
}

response_from!(AddProject);
response_from!(AddJob);
response_from!(GetJob);
response_from!(GetJobs);
response_from!(TakeJob);

macro_rules! gen_conv {
    ($name:ident, $ret:ty, $resptype:path) => {
        paste! {
            pub fn [<into_ $name>](self) -> Option<$ret> {
                if let $resptype(resp) = self {
                    Some(resp)
                } else {
                    None
                }
            }
        }
    };
}

impl Response {
    pub fn is_error(&self) -> bool {
        matches!(self, Response::BadRequest(_) | Response::NotFound |
                 Response::InternalError)
    }

    gen_conv!(add_project, AddProjectResponse, Response::AddProject);
    gen_conv!(add_job, AddJobResponse, Response::AddJob);
    gen_conv!(get_job, GetJobResponse, Response::GetJob);
    gen_conv!(get_jobs, GetJobsResponse, Response::GetJobs);
    gen_conv!(take_job, TakeJobResponse, Response::TakeJob);
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AddProjectRequest {
    pub name: String,
    pub heartbeat_expiration_millis: i32,
    pub data: serde_json::Value,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AddProjectResponse {
    pub project_id: ProjectId,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize, AsRefStr, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum JobState {
    Available,
    Running,
    Canceling,
    Canceled,
    Succeeded,
    Failed,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Job {
    pub id: JobId,
    pub project_name: String,
    pub project_id: ProjectId,
    pub state: JobState,
    pub created: DateTime<Utc>,
    pub started: Option<DateTime<Utc>>,
    pub finished: Option<DateTime<Utc>>,
    pub priority: i32,
    pub data: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetJobRequest {
    pub project_name: String,
    pub job_id: JobId,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct GetJobResponse {
    pub job: Job,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetJobsRequest {
    pub project_name: String,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct GetJobsResponse {
    pub jobs: Vec<Job>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AddJobRequest {
    pub project_name: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AddJobResponse {
    pub job_id: JobId,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TakeJobRequest {
    pub project_name: String,
    pub runner: String,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TakeJobResponseJob {
    pub job_id: JobId,
    pub job_token: JobToken,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TakeJobResponse {
    pub job: Option<TakeJobResponseJob>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateJobRequest {
    pub project_name: String,
    pub job_id: JobId,
    pub token: String,
    pub state: Option<JobState>,
    pub data: Option<serde_json::Value>,
}
