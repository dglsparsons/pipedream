#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use pipedream::app::*;
    use pipedream::fileserv::file_and_error_handler;
    use pipedream::workflow;
    use tokio::time::{sleep, Duration};

    _ = dotenvy::dotenv_override();

    simple_logger::init_with_level(log::Level::Info).expect("couldn't initialize logging");
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
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
            sleep(Duration::from_secs(5)).await
        }
    });

    let addr = leptos_options.site_addr;
    let app = Router::new()
        .leptos_routes(&leptos_options, routes, App)
        .fallback(file_and_error_handler)
        .with_state(leptos_options);

    if std::env::var("LOCAL_DEV").is_err() {
        let app = tower::ServiceBuilder::new()
            .layer(pipedream::vercel_axum::VercelLayer)
            .service(app);

        lambda_runtime::run(app).await.unwrap();
    } else {
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        log::info!("listening on http://{}", &addr);
        axum::serve(listener, app).await.unwrap();
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
}
