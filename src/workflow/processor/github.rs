use anyhow::Context;
use reqwest::{header, Client, StatusCode};
use serde::Serialize;
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
    pub token: &'a str,
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

pub async fn run_workflow(req: WorkflowRequest<'_>) -> Result<(), anyhow::Error> {
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
        .header(header::AUTHORIZATION, format!("Bearer {}", req.token))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .context("sending github workflow dispatch request")?;

    let status = res.status();
    let text = res
        .text()
        .await
        .unwrap_or_else(|_| "no error message".to_string());

    log::warn!(
        "workflow dispatch via github, status={}, text={}",
        status.clone().as_u16(),
        text
    );

    if status != StatusCode::NO_CONTENT {
        return Err(anyhow::anyhow!("failed to dispatch github workflow"));
    }

    Ok(())
}
