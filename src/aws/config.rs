use aws_config::AppName;
use aws_credential_types::Credentials;
use aws_types::{region::Region, sdk_config};
use std::env;
use tokio::sync::OnceCell;

pub async fn config() -> &'static sdk_config::SdkConfig {
    static CONFIG: OnceCell<sdk_config::SdkConfig> = OnceCell::const_new();
    let creds = Credentials::from_keys(
        env::var("PIPEDREAM_AWS_ACCESS_KEY_ID").unwrap(),
        env::var("PIPEDREAM_AWS_SECRET_ACCESS_KEY").unwrap(),
        None,
    );
    CONFIG
        .get_or_init(|| {
            aws_config::ConfigLoader::default()
                .behavior_version(aws_config::BehaviorVersion::v2024_03_28())
                .credentials_provider(creds)
                .region(Region::new(env::var("PIPEDREAM_AWS_REGION").unwrap()))
                .app_name(AppName::new("pipedream").unwrap())
                .load()
        })
        .await
}
