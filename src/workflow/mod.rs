use std::fmt::Display;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
mod client;

#[cfg(feature = "ssr")]
pub use client::*;

#[cfg(feature = "ssr")]
mod processor;

#[cfg(feature = "ssr")]
pub use processor::process_workflows;

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum Status {
    Paused,
    Running,
    Success,
    Failure,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Status::Paused => "Paused",
            Status::Running => "Running",
            Status::Success => "Success",
            Status::Failure => "Failure",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub enum EnvironmentStatus {
    Failure,
    Pending,
    Running,
    Success,
    Queued,
}

impl EnvironmentStatus {
    pub fn is_terminal(&self) -> bool {
        match self {
            EnvironmentStatus::Failure | EnvironmentStatus::Success => true,
            _ => false,
        }
    }
}

impl From<EnvironmentStatus> for Status {
    fn from(val: EnvironmentStatus) -> Self {
        match val {
            EnvironmentStatus::Pending => Status::Running,
            EnvironmentStatus::Running => Status::Running,
            EnvironmentStatus::Queued => Status::Running,
            EnvironmentStatus::Success => Status::Success,
            EnvironmentStatus::Failure => Status::Failure,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Environment {
    pub name: String,
    pub status: EnvironmentStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub deployment_id: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CreatedAt(DateTime<Utc>);

impl CreatedAt {
    fn to_rfc3339(&self) -> String {
        self.0.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    #[cfg(feature = "ssr")]
    fn now() -> Self {
        CreatedAt(Utc::now())
    }

    pub fn to_dt(&self) -> DateTime<Utc> {
        self.0
    }
}

impl Serialize for CreatedAt {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workflow {
    pub id: String,
    pub created_at: CreatedAt,
    pub updated_at: Option<DateTime<Utc>>,
    pub git_ref: String,
    pub owner: String,
    pub repo: String,
    pub sha: String,
    pub stability_period_minutes: usize,
    pub environments: Vec<Environment>,
    pub status: Status,
    pub commit_message: String,
    pub due_to_run: DateTime<Utc>,
}

impl Workflow {
    pub fn next_environment(&self) -> Option<(usize, &Environment)> {
        let idx = self
            .environments
            .iter()
            .position(|w| !w.status.is_terminal());

        idx.map(|i| (i, &self.environments[i]))
    }
}

#[cfg(feature = "ssr")]
pub struct CreateWorkflowRequest {
    pub git_ref: String,
    pub owner: String,
    pub repo: String,
    pub sha: String,
    pub stability_period_minutes: usize,
    pub environments: Vec<String>,
    pub commit_message: String,
}
