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

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum WaveStatus {
    Pending,
    Running,
    Success,
    Failure,
}

impl From<WaveStatus> for Status {
    fn from(val: WaveStatus) -> Self {
        match val {
            WaveStatus::Pending => Status::Running,
            WaveStatus::Running => Status::Running,
            WaveStatus::Success => Status::Success,
            WaveStatus::Failure => Status::Failure,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Environment {
    pub name: String,
    pub status: WaveStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workflow {
    pub id: String,
    pub created_at: DateTime<Utc>,
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
    pub fn next_wave(&self) -> Option<(usize, &Environment)> {
        let idx = self
            .environments
            .iter()
            .position(|w| w.status == WaveStatus::Running || w.status == WaveStatus::Pending);

        idx.and_then(|idx| self.environments.get(idx).map(|w| (idx, w)))
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
