use leptos_axum::generate_route_list;
use serde::{Deserialize, Serialize};
use server_fn::axum::server_fn_paths;
use std::collections::HashMap;
use std::fs::OpenOptions;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Locale {
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cookie: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
enum HasField {
    Host {
        value: String,
    },
    Header {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    Cookie {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    Query {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum HandleValue {
    Rewrite,
    Filesystem,
    Resource,
    Miss,
    Hit,
    #[default]
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", untagged)]
enum Route {
    Source {
        src: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        dest: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        methods: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        r#continue: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        case_sensitive: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        check: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
        #[serde(skip_serializing_if = "Option::is_none")]
        has: Option<Vec<HasField>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        missing: Option<Vec<HasField>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        locale: Option<Locale>,
        #[serde(skip_serializing_if = "Option::is_none")]
        middleware_raw_src: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        middleware_path: Option<String>,
    },
    Handler {
        handle: HandleValue,
        #[serde(skip_serializing_if = "Option::is_none")]
        src: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dest: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
    },
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ImagesConfig {}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WildcardConfig {}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OverrideConfig {}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CronsConfig {}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Config {
    version: u8,
    routes: Option<Vec<Route>>,
    // images: Option<ImagesConfig>,
    // wildcard: Option<WildcardConfig>,
    // overrides: Option<OverrideConfig>,
    // cache: Vec<String>,
    // crons: CronsConfig,
}

fn method_to_string(m: leptos_router::Method) -> String {
    match m {
        leptos_router::Method::Get => "get".to_string(),
        leptos_router::Method::Post => "post".to_string(),
        leptos_router::Method::Put => "put".to_string(),
        leptos_router::Method::Delete => "delete".to_string(),
        leptos_router::Method::Patch => "patch".to_string(),
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ServerlessFunctionConfig {
    handler: String,
    runtime: String,
    environment: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_duration: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supports_wrapper: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supports_response_streaming: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_path_map: Option<HashMap<String, String>>,
}

fn main() {
    std::fs::remove_dir_all(".vercel/output/functions").unwrap_or_default();
    std::fs::create_dir_all(".vercel/output/functions").unwrap();
    let config_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(".vercel/output/config.json")
        .expect("config file to be writeable");

    let mut routes = server_fn_paths()
        .map(|(route, method)| Route::Source {
            src: route.to_string(),
            dest: Some(route.to_string()),
            methods: Some(vec![method.to_string().to_lowercase()]),
            headers: None,
            r#continue: None,
            case_sensitive: None,
            check: None,
            status: None,
            has: None,
            missing: None,
            locale: None,
            middleware_raw_src: None,
            middleware_path: None,
        })
        .collect::<Vec<_>>();

    let mut ssr_routes = generate_route_list(pipedream::app::App)
        .into_iter()
        .map(|route| {
            let path = route.path().to_string();
            let methods = route
                .methods()
                .map(|m| method_to_string(m))
                .collect::<Vec<_>>();
            Route::Source {
                src: path.clone(),
                dest: Some(path),
                methods: Some(methods),
                headers: None,
                r#continue: None,
                case_sensitive: None,
                check: None,
                status: None,
                has: None,
                missing: None,
                locale: None,
                middleware_raw_src: None,
                middleware_path: None,
            }
        })
        .collect::<Vec<_>>();

    routes.append(&mut ssr_routes);
    routes.push(Route::Handler {
        handle: HandleValue::Filesystem,
        src: None,
        dest: None,
        status: None,
    });

    let config = Config {
        version: 3,
        routes: Some(routes.clone()),
    };

    serde_json::to_writer_pretty(config_file, &config).expect("config file to be written");
    for route in routes {
        if let Route::Source { src, .. } = route {
            std::fs::create_dir_all(format!(".vercel/output/functions/{}.func", src)).unwrap();
            let file_path = format!(".vercel/output/functions/{}.func/.vc-config.json", src);
            let func_file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(file_path.clone())
                .expect(&format!("{file_path} func file should be writeable"));

            let mut files = HashMap::new();
            files.insert(
                "bootstrap".to_string(),
                "target/lambda/pipedream/bootstrap".to_string(),
            );
            serde_json::to_writer_pretty(
                func_file,
                &ServerlessFunctionConfig {
                    handler: "bootstrap".to_string(),
                    runtime: "provided.al2023".to_string(),
                    architecture: Some("arm64".to_string()),
                    file_path_map: Some(files),
                    ..ServerlessFunctionConfig::default()
                },
            )
            .expect("func file to be written");
        } else {
            println!("skipping route {:?}", route);
        }
    }
}
