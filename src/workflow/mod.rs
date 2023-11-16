use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
mod client;

#[cfg(feature = "ssr")]
pub use client::*;

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum Status {
    Paused,
    Running,
    Success,
    Failure,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum WaveStatus {
    Pending,
    Running,
    Success,
    Failure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Wave {
    pub name: String,
    pub status: WaveStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workflow {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub github_token: String,
    pub git_ref: String,
    pub owner: String,
    pub repo: String,
    pub sha: String,
    pub stability_period_minutes: usize,
    pub waves: Vec<Wave>,
    pub workflow: String,
    pub status: Status,
    pub commit_message: String,
}

#[cfg(feature = "ssr")]
pub struct CreateWorkflowRequest {
    pub github_token: String,
    pub git_ref: String,
    pub owner: String,
    pub repo: String,
    pub sha: String,
    pub stability_period_minutes: usize,
    pub waves: Vec<String>,
    pub workflow: String,
    pub commit_message: String,
}
