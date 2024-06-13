use crate::error_template::{AppError, ErrorTemplate};
use crate::pages;
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
            view! { <ErrorTemplate outside_errors/> }.into_view()
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
    use http::{HeaderMap, StatusCode};
    use leptos::expect_context;
    use leptos_axum::{extract, ResponseOptions};

    let environments = environments
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    let response = expect_context::<ResponseOptions>();

    let headers: HeaderMap = extract().await?;
    let auth_header = headers.get("authorization").ok_or_else(|| {
        response.set_status(StatusCode::UNAUTHORIZED);
        ServerFnError::new("missing authorization header")
    })?;

    let token = auth_header.to_str().map_err(|e| {
        log::info!(
            "failed to get header value from header {auth_header:?}: {:#}",
            e
        );
        response.set_status(StatusCode::BAD_REQUEST);
        ServerFnError::new("invalid authorization header")
    })?;

    let token = if token.starts_with("Bearer ") {
        token.trim_start_matches("Bearer ")
    } else {
        token
    };

    crate::github::validate_oidc_token(token, &owner)
        .await
        .map_err(|e| {
            response.set_status(StatusCode::UNAUTHORIZED);
            log::info!("failed to validate token: {:#}", e);
            ServerFnError::new("invalid authorization token")
        })?;

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
