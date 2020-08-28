use chrono::{DateTime, Utc};
use paste::paste;
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, EnumString};

pub type JobId = i64;
pub type JobToken = String;
pub type ProjectId = i64;

macro_rules! into_request {
    ($name:ident, $variant:ident) => {
        impl From<$name> for Request {
            fn from(request: $name) -> Request {
                Request::$variant(request)
            }
        }

        impl $name {
            pub fn into_request(self) -> Request {
                Request::$variant(self)
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

into_request!(AddProjectRequest, AddProject);
into_request!(AddJobRequest, AddJob);
into_request!(GetJobRequest, GetJob);
into_request!(GetJobsRequest, GetJobs);
into_request!(TakeJobRequest, TakeJob);
into_request!(UpdateJobRequest, UpdateJob);

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Response {
    AddProject(AddProjectResponse),
    AddJob(AddJobResponse),
    GetJob(Job),
    GetJobs(Vec<Job>),
    TakeJob(Option<TakeJobResponse>),
    Empty,

    BadRequest(String),
    NotFound,
    InternalError,
}

macro_rules! gen_conv {
    ($name:ident, $ret:ty, $resptype:path) => {
        paste! {
            pub fn [<as_ $name>](&self) -> Option<&$ret> {
                if let $resptype(resp) = self {
                    Some(resp)
                } else {
                    None
                }
            }

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
    gen_conv!(get_job, Job, Response::GetJob);
    gen_conv!(get_jobs, Vec<Job>, Response::GetJobs);
    gen_conv!(take_job, Option<TakeJobResponse>, Response::TakeJob);
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

#[derive(Debug, Deserialize, Serialize)]
pub struct GetJobsRequest {
    pub project_name: String,
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
pub struct TakeJobResponse {
    pub job_id: JobId,
    pub job_token: JobToken,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateJobRequest {
    pub project_name: String,
    pub job_id: JobId,
    pub token: String,
    pub state: Option<JobState>,
    pub data: Option<serde_json::Value>,
}
