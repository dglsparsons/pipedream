use crate::error_template::{AppError, ErrorTemplate};
use crate::pages;
use http::HeaderMap;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Stylesheet id="leptos" href="/pkg/pipedream.css"/>
        <Title text="Pipedream"/>
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! {
                <ErrorTemplate outside_errors/>
            }
            .into_view()
        }>
            <main>
                <Routes>
                    <Route path="" view=pages::Home/>
                    <Route path="dashboard" view=pages::Dashboard/>
                </Routes>
            </main>
        </Router>
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    url: String,
}

#[server(CreateWorkflow, "/api", "Url", "workflow")]
pub async fn create_workflow(
    git_ref: String,
    repo: String,
    owner: String,
    sha: String,
    stability_period_minutes: usize,
    environments: String,
    commit_message: String,
) -> Result<Response, ServerFnError> {
    use super::workflow;
    use leptos_axum::extract;

    let environments = environments
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    let headers: HeaderMap = extract().await?;
    log::info!("headers: {:#?}", headers);

    workflow::client()
        .await
        .create(workflow::CreateWorkflowRequest {
            git_ref,
            repo: repo.clone(),
            owner: owner.clone(),
            sha: sha.clone(),
            stability_period_minutes,
            environments,
            commit_message,
        })
        .await
        .map_err(ServerFnError::new)?;

    Ok(Response {
        url: format!("https://pipedream.fly.dev/{}/{}/{}", owner, repo, sha),
    })
}

#[server(Authorize, "/api", "GetJson", "github/callback")]
pub async fn authorize(code: String) -> Result<(), ServerFnError> {
    use crate::github;
    use axum_extra::extract::cookie::{Cookie, SameSite};
    use http::header;
    use leptos::expect_context;
    use leptos_axum::ResponseOptions;
    use time::Duration;

    let response = expect_context::<ResponseOptions>();

    let auth_tokens = match github::exchange_oauth_token(&code).await {
        Err(e) => {
            log::error!("failed to exchange oauth token: {:#}", e);
            leptos_axum::redirect("/");
            Err(ServerFnError::new("unable to list workflows"))
        }
        Ok(v) => Ok(v),
    }?;

    let cookie = Cookie::build(("access", auth_tokens.access_token))
        .domain("127.0.0.1")
        .path("/")
        .secure(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::new(auth_tokens.expires_in - 30, 0))
        .http_only(true);

    if let Ok(cookie) = header::HeaderValue::from_str(&cookie.to_string()) {
        response.append_header(header::SET_COOKIE, cookie);
    }

    let cookie = Cookie::build(("refresh", auth_tokens.refresh_token))
        .domain("127.0.0.1")
        .path("/")
        .secure(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::days(10))
        .http_only(true);

    if let Ok(cookie) = header::HeaderValue::from_str(&cookie.to_string()) {
        response.append_header(header::SET_COOKIE, cookie);
    }

    leptos_axum::redirect("/dashboard");

    Ok(())
}
