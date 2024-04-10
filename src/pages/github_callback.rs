use http::header;
use leptos::*;
use leptos_router::{use_query, Params};
use time::Duration;

#[server(Authorize)]
pub async fn authorize(code: String) -> Result<(), ServerFnError> {
    use crate::github;
    use axum_extra::extract::cookie::{Cookie, SameSite};
    use leptos_axum::ResponseOptions;

    let auth_tokens = match github::exchange_oauth_token(&code).await {
        Err(e) => {
            log::error!("failed to exchange oauth token: {:#}", e);
            Err(ServerFnError::new("unable to list workflows"))
        }
        Ok(v) => Ok(v),
    }?;

    let response = expect_context::<ResponseOptions>();

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

    Ok(())
}

#[derive(Params, PartialEq, Debug, Default)]
struct CallbackQuery {
    code: String,
}

#[component]
pub fn GithubCallbackPage() -> impl IntoView {
    let query = use_query::<CallbackQuery>();
    let code = move || query.with(|q| q.as_ref().map(|q| q.code.clone()).unwrap_or_default());

    let result = create_resource(move || code(), |code| authorize(code));

    view! {
        <p>
            Code: {code}
        </p>
    }
}
