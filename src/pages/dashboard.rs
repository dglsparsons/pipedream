use crate::app::Logout;
use crate::workflow;
use crate::workflow::{Environment, EnvironmentStatus, Workflow};
use chrono::{DateTime, Local};
use leptos::*;
use leptos_meta::Title;
use leptos_router::{ActionForm, A};
use std::time::Duration;

#[server(ListWorkflows)]
pub async fn list_workflows(
    owner: String,
    repo: String,
) -> Result<Vec<workflow::Workflow>, ServerFnError> {
    if owner.is_empty() || repo.is_empty() {
        return Ok(vec![]);
    }

    // TODO - authentication middleware. Currently this is unauthenticated. .

    match workflow::client().await.list(owner, repo).await {
        Err(e) => {
            log::error!("failed to list workflows: {:#}", e);
            Err(ServerFnError::new("unable to list workflows"))
        }
        Ok(v) => Ok(v),
    }
}

#[server(ListRepos)]
pub async fn list_repos() -> Result<Vec<String>, ServerFnError> {
    use crate::github;
    use anyhow::Context;
    use http::StatusCode;
    use leptos_axum::extract;

    let response = expect_context::<leptos_axum::ResponseOptions>();
    let jar: axum_extra::extract::CookieJar = extract().await?;

    // TODO - refresh tokens, expired access tokens... :shrug:
    let access_token = match jar.get("access") {
        Some(v) => v,
        None => {
            log::info!("no access token found, redirecting to login");
            response.set_status(StatusCode::UNAUTHORIZED);
            leptos_axum::redirect("/");
            return Err(ServerFnError::new("not authorized"));
        }
    };

    let installations = github::list_user_installations(access_token.value())
        .await
        .context("listing user orgs")
        .map_err(ServerFnError::new)?;

    let futures =
        installations
            .into_iter()
            .map(|i| async move {
                github::list_installation_repositories(access_token.value(), i.id).await
            })
            .collect::<Vec<_>>();

    let mut repos = vec![];
    for f in futures {
        let x = f.await.map_err(ServerFnError::new)?;
        repos.extend(x);
    }

    Ok(repos)
}

#[component]
fn WorkflowCard(workflow: Workflow) -> impl IntoView {
    let local_time: DateTime<Local> = DateTime::from(workflow.created_at.to_dt());
    view! {
        <div class="rounded-lg border border-gray-600 bg-gray-700 text-card-foreground shadow-sm">
            <div class="p-6">
                <h2 class="text-xl font-bold mb-1">{workflow.commit_message}</h2>
                <p class="text-sm mb-1 font-extralight">
                    Created {format!("{}", local_time.format("%d %b, %Y, %H:%M"))}
                </p>
                <p
                    class="text-sm mb-6"
                    class=("text-green-500", move || workflow.status == workflow::Status::Success)
                    class=("text-red-500", move || workflow.status == workflow::Status::Failure)
                    class=("text-yellow-500", move || workflow.status == workflow::Status::Running)
                    class=("text-orange-500", move || workflow.status == workflow::Status::Paused)
                >

                    Status:
                    {format!("{}", workflow.status)}
                </p>
                <div class="flex flex-wrap justify-start gap-2">
                    <For
                        each=move || workflow.environments.clone().into_iter()
                        key=|w| w.name.clone()
                        children=move |w: Environment| {
                            let owner = workflow.owner.clone();
                            let repo = workflow.repo.clone();
                            let name = w.name.clone();
                            view! {
                                <a
                                    rel="external noopener"
                                    href=move || {
                                        format!(
                                            "https://github.com/{}/{}/deployments/{}",
                                            owner,
                                            repo,
                                            name,
                                        )
                                    }

                                    class="px-2 py-1 text-white rounded"
                                    class=(
                                        "bg-green-500",
                                        move || w.status == EnvironmentStatus::Success,
                                    )

                                    class=(
                                        "bg-green-500",
                                        move || w.status == EnvironmentStatus::Queued,
                                    )

                                    class=(
                                        "bg-red-500",
                                        move || w.status == EnvironmentStatus::Failure,
                                    )

                                    class=(
                                        "bg-yellow-500",
                                        move || w.status == EnvironmentStatus::Running,
                                    )

                                    class=(
                                        "bg-gray-500",
                                        move || w.status == EnvironmentStatus::Pending,
                                    )
                                >

                                    {w.name}
                                </a>
                            }
                        }
                    />

                </div>
            </div>
        </div>
    }
}

#[component]
fn Deployments(repo: ReadSignal<String>) -> impl IntoView {
    let workflows = create_resource(repo, |repo| {
        let parts = repo.split('/').collect::<Vec<_>>();
        let owner = parts.first().unwrap_or(&"").to_string();
        let repo = parts.get(1).unwrap_or(&"").to_string();
        list_workflows(owner, repo)
    });
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

    view! {
        <Transition fallback=move || view! { <p>"Loading..."</p> }>
            <main class="px-6 py-8 grid grid-cols-1 gap-4 gap-y-8">
                {move || {
                    workflows
                        .get()
                        .map(|w| match w {
                            Ok(w) => {
                                view! {
                                    <For
                                        each=move || w.clone()
                                        key=|w| w.id.clone()
                                        children=move |w: Workflow| {
                                            view! { <WorkflowCard workflow=w/> }
                                        }
                                    />
                                }
                                    .into_view()
                            }
                            Err(e) => {
                                view! { <p>Something went wrong: {format!("{}", e)}</p> }
                                    .into_view()
                            }
                        })
                }}

            </main>
        </Transition>
    }
}

#[component]
pub fn SelectOption(is: String, value: ReadSignal<String>) -> impl IntoView {
    let v = is.clone();
    view! {
        <option value=&v selected=move || value() == is>
            {v}
        </option>
    }
}

#[component]
pub fn Dashboard() -> impl IntoView {
    let (repo, set_repo) = create_signal("".to_string());
    let repos = create_local_resource(move || (), |_| list_repos());
    let logout = create_server_action::<Logout>();

    create_effect(move |_| {
        if repo.get().is_empty() {
            set_repo(
                repos
                    .get()
                    .and_then(|r| r.ok())
                    .and_then(|r| r.first().cloned())
                    .unwrap_or_default(),
            );
        }
    });

    view! {
        <div class="min-h-screen bg-gray-100 dark:bg-gray-800 dark:text-white">
            <header class="p-6 bg-white shadow dark:bg-gray-900">
                <div class="flex justify-between max-w-6xl mx-auto w-full">
                    <nav class="flex flex-row gap-8">
                        <A href="/dashboard" class="flex items-center">
                            Deployments
                        </A>
                        <A href="/environments" class="flex items-center">
                            Environments
                        </A>
                    </nav>
                    <ActionForm class="flex items-center gap-4" action=logout>
                        <button type="submit" class="text-sm">
                            Logout
                        </button>
                    </ActionForm>
                </div>
            </header>
            <main class="max-w-4xl mx-auto w-full">
                <div class="pt-12 px-6 flex flex-row justify-between">
                    <Transition fallback=move || {
                        view! {
                            <select class="min-w-24 max-w-48 flex h-10 w-full items-center justify-between rounded-md border border-input bg-white dark:bg-gray-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50">
                                "Loading..."
                            </select>
                        }
                    }>

                        {move || {
                            repos
                                .get()
                                .map(|repos| match repos {
                                    Ok(repos) => {
                                        view! {
                                            <select
                                                class="flex h-10 items-center justify-between rounded-md border border-input bg-white dark:bg-gray-900 px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                                                on:change=move |ev| {
                                                    let new_value = event_target_value(&ev);
                                                    set_repo(new_value);
                                                }
                                            >

                                                <For each=move || repos.clone() key=|r| r.clone() let:child>
                                                    <SelectOption is=child value=repo/>
                                                </For>
                                            </select>
                                        }
                                            .into_view()
                                    }
                                    Err(e) => {
                                        view! { <p>Something went wrong: {format!("{e}")}</p> }
                                            .into_view()
                                    }
                                })
                        }}

                    </Transition>
                    <button class="bg-rose-800 text-white font-semibold py-2 px-4 rounded hover:bg-rose-700 transition duration-300">
                        Stop Deployments
                    </button>
                    <Title text=repo/>
                </div>
                <Deployments repo=repo/>
            </main>
        </div>
    }
}
