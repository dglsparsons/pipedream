use leptos::*;
use leptos_router::{use_query, Params};

#[derive(Params, PartialEq, Debug, Default)]
struct CallbackQuery {
    code: String,
}

#[component]
pub fn GithubCallbackPage() -> impl IntoView {
    let query = use_query::<CallbackQuery>();
    let code = move || query.with(|q| q.as_ref().map(|q| q.code.clone()).unwrap_or_default());

    view! {
        <p>
            Code: {code}
        </p>
    }
}
