use leptos::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GithubInfo {
    client_id: String,
    redirect_uri: String,
}

#[server(GithubDetails)]
#[allow(clippy::unused_async)]
pub async fn github_info() -> Result<GithubInfo, ServerFnError> {
    use std::env;

    let domain = env::var("DOMAIN")
        .map(|d| format!("https://{d}"))
        .unwrap_or("http://127.0.0.1:3000".to_string());

    Ok(GithubInfo {
        client_id: env::var("GITHUB_CLIENT_ID")?,
        redirect_uri: format!("{domain}/api/github/callback"),
    })
}

#[component]
pub fn Home() -> impl IntoView {
    let github_client_id = create_resource(|| (), |_| github_info());
    view! {
        <div class="min-h-screen bg-gray-100 dark:bg-gray-800 dark:text-white">
          <header class="flex items-center justify-between p-6 bg-white shadow dark:bg-gray-900">
            <Suspense fallback=move || view! { <p>Log in with Github</p> }>
              {move || {
                github_client_id.get().map(|result| match result {
                  Ok(github_info) => view! {
                    <a class="cursor-pointer" href=format!("https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}", github_info.client_id, github_info.redirect_uri)>
                      Log in with Github
                    </a>
                  }.into_view(),
                  Err(_) => view! { <p></p> }.into_view(),
                })
              }}
            </Suspense>
          </header>
        </div>
    }
}
