use anyhow::Context;
use jsonwebtoken::{self, decode, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use reqwest::{header, Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::workflow::EnvironmentStatus;

mod token_cache;

use token_cache::token_cache;

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

const GITHUB_API_VERSION_HEADER: &str = "X-GitHub-Api-Version";
const GITHUB_API_VERSION: &str = "2022-11-28";

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
    #[allow(dead_code)]
    total_count: i64,
    workflow_runs: Vec<Workflow>,
}

async fn get_token(owner: &str, repo: &str) -> Result<String, anyhow::Error> {
    let tc = token_cache();
    let mut tc = tc.lock().await;
    tc.get_or_create(owner, repo).await.context("getting token")
}

pub async fn list_workflows(
    owner: &str,
    repo: &str,
    sha: &str,
    event: &str,
) -> Result<Vec<Workflow>, anyhow::Error> {
    let token = get_token(owner, repo).await?;
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
    req: CreateDeploymentRequest<'_>,
) -> Result<CreateDeploymentResponse, anyhow::Error> {
    let token = get_token(req.owner, req.repo).await?;
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
            EnvironmentStatus::Pending | EnvironmentStatus::Queued => DeploymentStatus::Queued,
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
    owner: &str,
    repo: &str,
    deployment_id: &u64,
    status: DeploymentStatus,
) -> Result<(), anyhow::Error> {
    let token = get_token(owner, repo).await?;
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
#[serde(untagged)]
enum OAuthResponse {
    Success(OauthTokenResponse),
    Error(OAuthErrorResponse),
}

#[derive(Debug, Deserialize, Default)]
struct OAuthErrorResponse {
    error: String,
    error_description: String,
    error_uri: String,
}

#[derive(Debug, Deserialize, Default)]
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
        .json::<OAuthResponse>()
        .await
        .context("parsing github access_token response")?;

    match response {
        OAuthResponse::Success(token) => Ok(token),
        OAuthResponse::Error(e) => Err(anyhow::anyhow!(
            "failed to exchange oauth token: {} - {:#}: {}",
            e.error,
            e.error_description,
            e.error_uri,
        )),
    }
}

#[derive(Debug, Deserialize)]
struct Repository {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct ListRespositoriesResponse {
    #[allow(dead_code)]
    total_count: i64,
    repositories: Vec<Repository>,
}

pub async fn list_installation_repositories(
    token: &str,
    installation_id: i64,
) -> Result<Vec<String>, anyhow::Error> {
    let res = http()
        .await
        .get(format!(
            "https://api.github.com/user/installations/{}/repositories",
            installation_id
        ))
        .query(&[("per_page", "100")])
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("listing installation repositories")?;

    let status = res.status();
    if status != StatusCode::OK {
        let text = res
            .text()
            .await
            .unwrap_or_else(|_| "no error message".to_string());

        log::info!(
            "failed to list installation repositories, status={}, text={}",
            status.clone().as_u16(),
            text
        );
        return Err(anyhow::anyhow!("failed to list installition repositories"));
    }

    let response = res
        .json::<ListRespositoriesResponse>()
        .await
        .context("parsing github list repositories response")?;

    Ok(response
        .repositories
        .into_iter()
        .map(|r| r.full_name)
        .collect())
}

#[derive(Debug, Deserialize)]
pub struct InstallationAccount {
    #[allow(dead_code)]
    id: i64,
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct Installation {
    pub id: i64,
    pub account: InstallationAccount,
}

#[derive(Debug, Deserialize)]
struct UserInstallationResponse {
    #[allow(dead_code)]
    total_count: i64,
    installations: Vec<Installation>,
}

pub async fn list_user_installations(token: &str) -> Result<Vec<Installation>, anyhow::Error> {
    let res = http()
        .await
        .get("https://api.github.com/user/installations")
        .query(&[("per_page", "100")])
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("listing user installations")?;

    let status = res.status();
    if status != StatusCode::OK {
        let text = res
            .text()
            .await
            .unwrap_or_else(|_| "no error message".to_string());

        log::info!(
            "failed to list installation repositories, status={}, text={}",
            status.clone().as_u16(),
            text
        );
        return Err(anyhow::anyhow!("failed to list installition repositories"));
    }

    let response = res
        .json::<UserInstallationResponse>()
        .await
        .context("parsing github list repositories response")?;

    Ok(response.installations)
}

#[derive(Debug, Deserialize)]
struct JWKResponse {
    keys: Vec<jsonwebtoken::jwk::Jwk>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    aud: String,
    iss: String,
}

pub async fn validate_oidc_token(token: &str, owner: &str) -> Result<(), anyhow::Error> {
    let res = http()
        .await
        .get("https://token.actions.githubusercontent.com/.well-known/jwks")
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/json")
        .send()
        .await
        .context("getting github jwks")?;

    let status = res.status();
    if status != StatusCode::OK {
        let text = res
            .text()
            .await
            .unwrap_or_else(|_| "no error message".to_string());

        log::info!(
            "failed to load github JWKs , status={}, text={}",
            status.clone().as_u16(),
            text
        );
        return Err(anyhow::anyhow!("failed to list github JWKs"));
    }
    let res = res
        .json::<JWKResponse>()
        .await
        .context("parsing github JWK response")?;

    let jwks = JwkSet { keys: res.keys };

    let key = jsonwebtoken::decode_header(token)
        .context("decoding header")
        .and_then(|header| header.kid.ok_or_else(|| anyhow::anyhow!("missing kid")))
        .and_then(|kid| {
            jwks.find(&kid)
                .ok_or_else(|| anyhow::anyhow!("no key found matching header"))
        })
        .and_then(|k| DecodingKey::from_jwk(k).context("getting decoding key from jwk"))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://token.actions.githubusercontent.com"]);
    validation.set_audience(&[format!("https://github.com/{owner}")]);

    decode::<Claims>(token, &key, &validation).context("decoding token")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::JWKResponse;

    #[tokio::test]
    async fn test_decoding_jwk() {
        let raw = r#"{
  "keys": [
    {
      "kty": "RSA",
      "alg": "RS256",
      "use": "sig",
      "kid": "cc413527-173f-5a05-976e-9c52b1d7b431",
      "n": "w4M936N3ZxNaEblcUoBm-xu0-V9JxNx5S7TmF0M3SBK-2bmDyAeDdeIOTcIVZHG-ZX9N9W0u1yWafgWewHrsz66BkxXq3bscvQUTAw7W3s6TEeYY7o9shPkFfOiU3x_KYgOo06SpiFdymwJflRs9cnbaU88i5fZJmUepUHVllP2tpPWTi-7UA3AdP3cdcCs5bnFfTRKzH2W0xqKsY_jIG95aQJRBDpbiesefjuyxcQnOv88j9tCKWzHpJzRKYjAUM6OPgN4HYnaSWrPJj1v41eEkFM1kORuj-GSH2qMVD02VklcqaerhQHIqM-RjeHsN7G05YtwYzomE5G-fZuwgvQ",
      "e": "AQAB"
    },
  ]
}"#;

        let v: Result<JWKResponse, _> = serde_json::from_str(raw);
        assert!(v.is_ok());
    }
}
