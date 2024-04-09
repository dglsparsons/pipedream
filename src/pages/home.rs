use crate::workflow;
use crate::workflow::{Environment, EnvironmentStatus, Workflow};
use chrono::{DateTime, Local};
use leptos::*;
use leptos_meta::Title;
use std::time::Duration;

#[server(ListWorkflows)]
pub async fn list_workflows(
    owner: String,
    repo: String,
) -> Result<Vec<workflow::Workflow>, ServerFnError> {
    match workflow::client().await.list(owner, repo).await {
        Err(e) => {
            log::error!("failed to list workflows: {:#}", e);
            Err(ServerFnError::new("unable to list workflows"))
        }
        Ok(v) => Ok(v),
    }
}

#[component]
fn WorkflowCard(workflow: Workflow) -> impl IntoView {
    let local_time: DateTime<Local> = DateTime::from(workflow.created_at.to_dt());
    view! {
        <div class="rounded-lg border bg-card text-card-foreground shadow-sm">
          <div class="p-6">
            <h2 class="text-xl font-bold mb-2">{workflow.commit_message}</h2>
            <p class="text-sm mb-4">Created {format!("{}", local_time.format("%d %b, %Y, %H:%M"))}</p>
            <p class="text-green-500 mb-4">Status: {format!("{}", workflow.status)}</p>
            <h3 class="text-lg font-bold">Environments:</h3>
            <div class="flex flex-wrap justify-start gap-2">
            <For
              each=move || workflow.environments.clone().into_iter()
              key=|w| w.name.clone()
              children=move |w: Environment| {
                  view! {
                      <span
                          class="px-2 py-1 text-white rounded"
                          class=("bg-green-500", move || w.status == EnvironmentStatus::Success)
                          class=("bg-red-500", move || w.status == EnvironmentStatus::Failure)
                          class=("bg-yellow-500", move || w.status == EnvironmentStatus::Running)
                          class=("bg-gray-500", move || w.status == EnvironmentStatus::Pending)
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

#[component]
pub fn HomePage() -> impl IntoView {
    let client_id = std::env::var("GITHUB_CLIENT_ID").unwrap();
    view! {
        <div class="min-h-screen bg-gray-100 dark:bg-gray-800 dark:text-white">
          <header class="flex items-center justify-between p-6 bg-white shadow dark:bg-gray-900">
            <div class="flex items-center">
              <h1 class="text-2xl font-bold mr-4">Deployments</h1>
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
            <a class="cursor-pointer" href=format!("https://github.com/login/oauth/authorize?client_id={client_id}")>
            Log in with Github
            </a>
          </header>
          <Deployments/>
        </div>
    }
}
