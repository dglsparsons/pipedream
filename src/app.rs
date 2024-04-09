use super::workflow;
use crate::error_template::{AppError, ErrorTemplate};
use crate::pages::*;
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
                    <Route path="" view=HomePage/>
                    <Route path="github/callback" view=GithubCallbackPage/>
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
    let environments = environments
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
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
        .map_err(|e| ServerFnError::new(e))?;

    Ok(Response {
        url: format!("https://pipedream.fly.dev/{}/{}/{}", owner, repo, sha),
    })
}
