#![allow(clippy::too_many_arguments)]
use super::workflow;
use crate::{
    error_template::{AppError, ErrorTemplate},
    workflow::{Wave, WaveStatus, Workflow},
};
use chrono::{DateTime, Local};
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    github_token: String,
    git_ref: String,
    repo: String,
    owner: String,
    sha: String,
    stability_period_minutes: usize,
    waves: String,
    workflow: String,
    commit_message: String,
) -> Result<Response, ServerFnError> {
    let waves = waves
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    workflow::client()
        .await
        .create(workflow::CreateWorkflowRequest {
            github_token,
            git_ref,
            repo: repo.clone(),
            owner: owner.clone(),
            sha: sha.clone(),
            stability_period_minutes,
            waves,
            workflow,
            commit_message,
        })
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    Ok(Response {
        url: format!("https://pipedream.fly.dev/{}/{}/{}", owner, repo, sha),
    })
}

#[server(ListWorkflows)]
pub async fn list_workflows(
    owner: String,
    repo: String,
) -> Result<Vec<workflow::Workflow>, ServerFnError> {
    match workflow::client().await.list(owner, repo).await {
        Err(e) => {
            logging::error!("failed to list workflows: {:#}", e);
            Err(ServerFnError::ServerError(
                "unable to list workflows".to_string(),
            ))
        }
        Ok(v) => Ok(v),
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div class="min-h-screen bg-gray-100 dark:bg-gray-800 dark:text-white">
          <header class="flex items-center justify-between p-6 bg-white shadow dark:bg-gray-900">
            <div class="flex items-center">
              <h1 class="text-2xl font-bold mr-4">CI Deployments</h1>
              <button
                type="button"
                role="combobox"
                aria-controls="radix-:r1k:"
                aria-expanded="false"
                aria-autocomplete="none"
                dir="ltr"
                data-state="closed"
                data-placeholder=""
                class="flex h-10 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <span style="pointer-events: none;">Select a Repository</span>
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="24"
                  height="24"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  class="h-4 w-4 opacity-50"
                  aria-hidden="true"
                >
                  <path d="m6 9 6 6 6-6"></path>
                </svg>
              </button>
            </div>
          </header>
          <Deployments/>
        </div>
    }
}

#[component]
fn WorkflowCard(workflow: Workflow) -> impl IntoView {
    let local_time: DateTime<Local> = DateTime::from(workflow.created_at);
    view! {
        <div class="rounded-lg border bg-card text-card-foreground shadow-sm">
          <div class="p-6">
            <h2 class="text-xl font-bold mb-2">{workflow.commit_message}</h2>
            <p class="text-sm mb-4">Created {format!("{}", local_time.format("%d %b, %Y, %H:%M"))}</p>
            <p class="text-green-500 mb-4">Status: {format!("{}", workflow.status)}</p>
            <h3 class="text-lg font-bold">Environments:</h3>
            <div class="flex flex-wrap justify-start gap-2">
            <For
              each=move || workflow.waves.clone().into_iter()
              key=|w| w.name.clone()
              children=move |w: Wave| {
                  view! {
                      <span
                          class="px-2 py-1 text-white rounded"
                          class=("bg-green-500", move || w.status == WaveStatus::Success)
                          class=("bg-red-500", move || w.status == WaveStatus::Failure)
                          class=("bg-yellow-500", move || w.status == WaveStatus::Running)
                          class=("bg-gray-500", move || w.status == WaveStatus::Pending)
                      >
                          {w.name}
                      </span>
                  }
              }
            />
            </div>
          </div>
        </div>
    }
}

#[component]
fn Deployments() -> impl IntoView {
    let (owner, _set_owner) = create_signal("dglsparsons".to_string());
    let (repo, _set_repo) = create_signal("deploy-testing".to_string());
    let workflows = create_resource(
        move || (owner(), repo()),
        |(owner, repo)| list_workflows(owner, repo),
    );
    create_effect(move |_| {
        let handle = set_interval_with_handle(
            move || {
                workflows.refetch();
            },
            Duration::from_secs(5),
        )
        .expect("interval to be created");

        on_cleanup(move || {
            handle.clear();
        })
    });

    let title = move || format!("{}/{}", owner(), repo());
    view! {
        <Title text={title}/>
        <Transition
            fallback=move || view! { <p>"Loading..."</p> }
        >
            <main class="p-6 grid grid-cols-1 gap-4">
            {
                move || workflows.get().map(|w| match w {
                    Ok(w) => {
                        view! {
                            <For
                              each=move || w.clone()
                              key=|w| w.id.clone()
                              children=move |w: Workflow| {
                                  view! {
                                      <WorkflowCard workflow=w/>
                                  }
                              }
                            />
                        }.into_view()
                    },
                    Err(e) => {
                        view! {
                            <p>Something went wrong: {format!("{}", e)}</p>
                        }.into_view()
                    },
                })
            }
            </main>
        </Transition>
    }
}
