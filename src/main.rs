#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{routing::post, Router};
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use pipedream::app::*;
    use pipedream::fileserv::file_and_error_handler;
    use pipedream::workflow;
    use tokio::time::{sleep, Duration};

    simple_logger::init_with_level(log::Level::Info).expect("couldn't initialize logging");
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    tokio::spawn(async move {
        let client = workflow::client().await;
        if std::env::var("LEPTOS_WORKER").is_err() {
            log::warn!("not starting worker due to missing LEPTOS_WORKER env");
            return;
        }
        loop {
            if let Err(e) = workflow::process_workflows(client).await {
                log::error!("error processing workflows: {0:#}", e);
            }
            sleep(Duration::from_secs(10)).await
        }
    });

    let app = Router::new()
        .route("/api/*fn_name", post(leptos_axum::handle_server_fns))
        .leptos_routes(&leptos_options, routes, App)
        .fallback(file_and_error_handler)
        .with_state(leptos_options);

    log::info!("listening on http://{}", &addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
}
