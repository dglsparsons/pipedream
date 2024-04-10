use anyhow::Context;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::{header, Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::workflow::EnvironmentStatus;

async fn new_client() -> Client {
    Client::new()
}

async fn http() -> &'static Client {
    static CONFIG: OnceCell<Client> = OnceCell::const_new();
    CONFIG.get_or_init(new_client).await
}

pub struct CreateDeploymentRequest<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub git_ref: &'a str,
    pub environment: &'a str,
    pub description: &'a str,
}

#[derive(Debug, Serialize)]
struct CreateDeploymentRequestBody<'a> {
    r#ref: &'a str,
    environment: &'a str,
    description: &'a str,
    auto_merge: bool,
    required_contexts: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iat: i64,
    exp: i64,
    iss: &'static str,
    alg: &'static str,
}

const GITHUB_APP_ID: &str = "673610";
const ALG: &str = "RSA256";
const GITHUB_API_VERSION_HEADER: &str = "X-GitHub-Api-Version";
const GITHUB_API_VERSION: &str = "2022-11-28";

async fn generate_jwt() -> Result<String, anyhow::Error> {
    let iat = chrono::Utc::now() - chrono::Duration::seconds(60);
    let exp = iat + chrono::Duration::minutes(10);
    let my_claims = Claims {
        iat: iat.timestamp(),
        exp: exp.timestamp(),
        iss: GITHUB_APP_ID,
        alg: ALG,
    };
    let token = encode(
        &Header::new(Algorithm::RS256),
        &my_claims,
        &EncodingKey::from_rsa_pem(include_bytes!(
            "../../pipedream-ci.2024-03-01.private-key.pem"
        ))
        .context("creating encoding key from RSA pem")?,
    )
    .context("encoding JWT token")?;
    Ok(token)
}

#[derive(Debug, Deserialize)]
struct Installation {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct InstallationAccessToken {
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_access_token(org: String, repo: String) -> Result<String, anyhow::Error> {
    let token = generate_jwt().await?;

    let res = http()
        .await
        .get(format!(
            "https://api.github.com/repos/{}/{}/installation",
            org, repo
        ))
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("getting github installation id")?;

    let status = res.status();

    if status != StatusCode::OK {
        let text = res
            .text()
            .await
            .unwrap_or_else(|_| "no error message".to_string());

        log::info!(
            "get github installation id, status={}, text={}",
            status.clone().as_u16(),
            text
        );
        return Err(anyhow::anyhow!("failed to get github installation id"));
    }

    let installation: Installation = res
        .json()
        .await
        .context("parsing github installation id response")?;

    let res = http()
        .await
        .post(format!(
            "https://api.github.com/app/installations/{}/access_tokens",
            installation.id
        ))
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("creating github installation access token")?;

    let status = res.status();

    if status != StatusCode::CREATED {
        let text = res
            .text()
            .await
            .unwrap_or_else(|_| "no error message".to_string());

        log::info!(
            "creating github installation access token, status={}, text={}",
            status.clone().as_u16(),
            text
        );
        return Err(anyhow::anyhow!("failed to get github installation id"));
    }

    let access_token: InstallationAccessToken = res
        .json()
        .await
        .context("parsing github installation access token response")?;

    log::info!(
        "created github installation access token, access_token={}, expires_at={}",
        access_token.token,
        access_token.expires_at,
    );

    return Ok(access_token.token);
}

// Github actions has way too many statuses, holy crap.
#[derive(Debug, Deserialize)]
pub enum WorkflowStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "action_required")]
    ActionRequired,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "failure")]
    Failure,
    #[serde(rename = "neutral")]
    Neutral,
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "timed_out")]
    TimedOut,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "requested")]
    Requested,
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "pending")]
    Pending,
}

#[derive(Debug, Deserialize)]
pub struct Workflow {
    pub id: u64,
    pub name: String,
    pub head_sha: String,
    pub head_branch: String,
    pub event: String,
    pub status: WorkflowStatus,
}

#[derive(Debug, Deserialize)]
struct ListWorkflowResponse {
    total_count: i64,
    workflow_runs: Vec<Workflow>,
}

pub async fn list_workflows(
    token: &str,
    owner: &str,
    repo: &str,
    sha: &str,
    event: &str,
) -> Result<Vec<Workflow>, anyhow::Error> {
    let res = http()
        .await
        .get(format!(
            "https://api.github.com/repos/{}/{}/actions/runs",
            owner, repo
        ))
        .query(&[("head_sha", sha), ("per_page", "100"), ("event", event)])
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("getting github workflow runs")?;

    let status = res.status();
    log::info!("listing workflows, status={}", status.clone().as_u16());

    if status != StatusCode::OK {
        let text = res
            .text()
            .await
            .unwrap_or_else(|_| "no error message".to_string());
        log::error!("failed to list workflows, status={}, text={}", status, text);
        return Err(anyhow::anyhow!("failed to dispatch github workflow"));
    }

    let response = res
        .json::<ListWorkflowResponse>()
        .await
        .context("parsing github workflow runs response")?;

    // TODO - pagination :oh_no:
    Ok(response.workflow_runs)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDeploymentResponse {
    pub id: u64,
}

pub async fn create_deployment(
    token: &str,
    req: CreateDeploymentRequest<'_>,
) -> Result<CreateDeploymentResponse, anyhow::Error> {
    let res = http()
        .await
        .post(format!(
            "https://api.github.com/repos/{}/{}/deployments?auto_merge=false",
            req.owner, req.repo,
        ))
        .json(&CreateDeploymentRequestBody {
            description: req.description,
            environment: req.environment,
            r#ref: req.git_ref,
            auto_merge: false,
            required_contexts: vec![],
        })
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("sending github workflow dispatch request")?;

    let status = res.status();
    if status != StatusCode::ACCEPTED && status != StatusCode::CREATED {
        let text = res
            .text()
            .await
            .unwrap_or_else(|_| "no error message".to_string());

        log::info!(
            "failed to create deployment, status={}, text={}",
            status.clone().as_u16(),
            text
        );
        return Err(anyhow::anyhow!("failed to dispatch github workflow"));
    }

    let response = res
        .json::<CreateDeploymentResponse>()
        .await
        .context("parsing github deployment response")?;

    Ok(response)
}

#[derive(Debug, Serialize)]
pub enum DeploymentStatus {
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "failure")]
    Failure,
    #[serde(rename = "success")]
    Success,
}

impl From<EnvironmentStatus> for DeploymentStatus {
    fn from(status: EnvironmentStatus) -> Self {
        match status {
            EnvironmentStatus::Pending => DeploymentStatus::Queued,
            EnvironmentStatus::Queued => DeploymentStatus::Queued,
            EnvironmentStatus::Running => DeploymentStatus::InProgress,
            EnvironmentStatus::Success => DeploymentStatus::Success,
            EnvironmentStatus::Failure => DeploymentStatus::Failure,
        }
    }
}

#[derive(Debug, Serialize)]
struct UpdateStatusRequestBody {
    state: DeploymentStatus,
}

pub async fn update_deployment_status(
    token: &str,
    owner: &str,
    repo: &str,
    deployment_id: &u64,
    status: DeploymentStatus,
) -> Result<(), anyhow::Error> {
    let res = http()
        .await
        .post(format!(
            "https://api.github.com/repos/{}/{}/deployments/{}/statuses",
            owner, repo, deployment_id
        ))
        .json(&UpdateStatusRequestBody { state: status })
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("updating deployment status")?;

    let status = res.status();
    let text = res
        .text()
        .await
        .unwrap_or_else(|_| "no error message".to_string());

    if status != StatusCode::CREATED {
        log::info!(
            "failed to update deployment status for {owner}, {repo}, {deployment_id}, status={}, text={text}",
            status.clone().as_u16()
        );
        return Err(anyhow::anyhow!("failed to update github deployment status"));
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct OauthTokenResponse {
    pub access_token: String,
    pub expires_in: i64, // number of seconds until expiration
    pub refresh_token: String,
    pub refresh_token_expires_in: u64,
    pub scope: String,
    pub token_type: String,
}

pub async fn exchange_oauth_token(code: &str) -> Result<OauthTokenResponse, anyhow::Error> {
    let client_id = std::env::var("GITHUB_CLIENT_ID").unwrap();
    let client_secret = std::env::var("GITHUB_CLIENT_SECRET").unwrap();
    let res = http()
        .await
        .post("https://github.com/login/oauth/access_token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code.to_owned()),
        ])
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/json")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("exchanging oauth token")?;

    let status = res.status();
    if status != StatusCode::OK {
        let text = res
            .text()
            .await
            .unwrap_or_else(|_| "no error message".to_string());

        log::info!(
            "failed to exchange oauth token, status={}, text={}",
            status.clone().as_u16(),
            text
        );
        return Err(anyhow::anyhow!("failed to exchange oauth token"));
    }

    let response = res
        .json::<OauthTokenResponse>()
        .await
        .context("parsing github access_token response")?;

    Ok(response)
}
