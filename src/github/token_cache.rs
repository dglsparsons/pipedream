use anyhow::Context;
use chrono::{DateTime, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::{header, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::Mutex;

pub(super) fn token_cache() -> &'static Mutex<TokenCache> {
    static CONFIG: OnceLock<Mutex<TokenCache>> = OnceLock::new();
    CONFIG.get_or_init(|| Mutex::new(TokenCache::new()))
}

pub(super) struct TokenCache(HashMap<(String, String), (DateTime<Utc>, String)>);

impl TokenCache {
    fn new() -> Self {
        TokenCache(HashMap::new())
    }

    pub(super) async fn get_or_create(
        &mut self,
        owner: &str,
        repo: &str,
    ) -> Result<String, anyhow::Error> {
        let key = (owner.to_string(), repo.to_string());
        let token = self.0.get(&key).and_then(|(exp, token)| {
            if exp < &Utc::now() {
                None
            } else {
                Some(token.as_str())
            }
        });

        match token {
            Some(t) => Ok(t.to_owned()),
            None => {
                let (exp, token) = create_access_token(owner.to_string(), repo.to_string())
                    .await
                    .context("creating access token")?;
                self.0.insert(key, (exp, token.clone()));
                Ok(token)
            }
        }
    }
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

fn generate_jwt() -> Result<String, anyhow::Error> {
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

async fn create_access_token(
    org: String,
    repo: String,
) -> Result<(DateTime<Utc>, String), anyhow::Error> {
    let token = generate_jwt()?;

    let res = super::http()
        .await
        .get(format!(
            "https://api.github.com/repos/{}/{}/installation",
            org, repo
        ))
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(super::GITHUB_API_VERSION_HEADER, super::GITHUB_API_VERSION)
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

    let res = super::http()
        .await
        .post(format!(
            "https://api.github.com/app/installations/{}/access_tokens",
            installation.id
        ))
        .header(header::USER_AGENT, "pipedream")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(super::GITHUB_API_VERSION_HEADER, super::GITHUB_API_VERSION)
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

    Ok((access_token.expires_at, access_token.token))
}
