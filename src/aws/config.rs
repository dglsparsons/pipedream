use aws_types::sdk_config;
use tokio::sync::OnceCell;

pub async fn config() -> &'static sdk_config::SdkConfig {
    static CONFIG: OnceCell<sdk_config::SdkConfig> = OnceCell::const_new();
    CONFIG.get_or_init(aws_config::load_from_env).await
}
