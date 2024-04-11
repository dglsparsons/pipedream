use leptos::*;

#[component]
pub fn Home() -> impl IntoView {
    let client_id = std::env::var("GITHUB_CLIENT_ID").unwrap();
    view! {
        <div class="min-h-screen bg-gray-100 dark:bg-gray-800 dark:text-white">
          <header class="flex items-center justify-between p-6 bg-white shadow dark:bg-gray-900">
            <a class="cursor-pointer" href=format!("https://github.com/login/oauth/authorize?client_id={client_id}")>
            Log in with Github
            </a>
          </header>
        </div>
    }
}
