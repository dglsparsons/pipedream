use anyhow::Context;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::{header, Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

async fn new_client() -> Client {
    Client::new()
}

async fn http() -> &'static Client {
    static CONFIG: OnceCell<Client> = OnceCell::const_new();
    CONFIG.get_or_init(new_client).await
}

pub struct WorkflowRequest<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub workflow: &'a str,
    pub wave: &'a str,
    pub git_ref: &'a str,
    pub sha: &'a str,
}

#[derive(Debug, Serialize)]
struct Inputs<'a> {
    wave: &'a str,
}

#[derive(Debug, Serialize)]
struct RequestBody<'a> {
    #[serde(rename = "ref")]
    git_ref: &'a str,
    inputs: Inputs<'a>,
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
            "../../../../pipedream-ci.2024-03-01.private-key.pem"
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

pub async fn run_workflow(req: WorkflowRequest<'_>) -> Result<(), anyhow::Error> {
    let access_token = create_access_token(req.owner.to_string(), req.repo.to_string())
        .await
        .context("creating access token")?;

    let res = http()
        .await
        .post(format!(
            "https://api.github.com/repos/{}/{}/actions/workflows/{}/dispatches",
            req.owner, req.repo, req.workflow
        ))
        .json(&RequestBody {
            git_ref: req.git_ref,
            inputs: Inputs { wave: req.wave },
        })
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", access_token))
        .header(GITHUB_API_VERSION_HEADER, GITHUB_API_VERSION)
        .send()
        .await
        .context("sending github workflow dispatch request")?;

    let status = res.status();
    let text = res
        .text()
        .await
        .unwrap_or_else(|_| "no error message".to_string());

    log::info!(
        "workflow dispatch via github, status={}, text={}",
        status.clone().as_u16(),
        text
    );

    if status != StatusCode::NO_CONTENT {
        return Err(anyhow::anyhow!("failed to dispatch github workflow"));
    }

    Ok(())
}
