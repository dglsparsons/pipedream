use leptos::*;

#[server(Logout)]
#[allow(clippy::unused_async)]
pub async fn logout() -> Result<(), ServerFnError> {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    use http::header;
    use leptos::expect_context;
    use time::OffsetDateTime;

    let response = expect_context::<leptos_axum::ResponseOptions>();
    let cookie = Cookie::build(("access", ""))
        .path("/")
        .secure(true)
        .same_site(SameSite::Strict)
        .expires(OffsetDateTime::UNIX_EPOCH)
        .http_only(true);

    if let Ok(cookie) = header::HeaderValue::from_str(&cookie.to_string()) {
        response.append_header(header::SET_COOKIE, cookie);
    }

    let cookie = Cookie::build(("refresh", ""))
        .path("/")
        .secure(true)
        .same_site(SameSite::Strict)
        .expires(OffsetDateTime::UNIX_EPOCH)
        .http_only(true);

    if let Ok(cookie) = header::HeaderValue::from_str(&cookie.to_string()) {
        response.append_header(header::SET_COOKIE, cookie);
    }

    leptos_axum::redirect("/");

    Ok(())
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

    let domain = std::env::var("DOMAIN").unwrap_or("127.0.0.1".to_string());
    let cookie = Cookie::build(("access", auth_tokens.access_token))
        .domain(&domain)
        .path("/")
        .secure(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::new(auth_tokens.expires_in - 30, 0))
        .http_only(true);

    if let Ok(cookie) = header::HeaderValue::from_str(&cookie.to_string()) {
        response.append_header(header::SET_COOKIE, cookie);
    }

    let cookie = Cookie::build(("refresh", auth_tokens.refresh_token))
        .domain(domain)
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
