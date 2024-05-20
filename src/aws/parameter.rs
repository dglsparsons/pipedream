use anyhow::{Context, Error};
use aws_sdk_ssm::Client;
use aws_types::sdk_config;

pub struct ParameterClient {
    client: Client,
}

#[derive(thiserror::Error, Debug)]
pub enum ParameterError {
    #[error("The requested item could not be found")]
    NotFound(),
    #[error("An unexpected error occurred: {0}")]
    Unknown(Error),
}

impl ParameterClient {
    pub fn new(config: &sdk_config::SdkConfig) -> ParameterClient {
        ParameterClient {
            client: Client::new(config),
        }
    }

    pub async fn get_parameter(&self, name: &str) -> Result<String, ParameterError> {
        let res = self
            .client
            .get_parameter()
            .name(name)
            .with_decryption(true)
            .send()
            .await
            .with_context(|| format!("failed to get parameter {}", name))
            .map_err(ParameterError::Unknown)?;

        let value = res.parameter().ok_or(ParameterError::NotFound())?;
        let value = value.value().ok_or(ParameterError::NotFound())?;
        Ok(value.to_string())
    }

    pub async fn get_parameters(&self, names: &[&str]) -> Result<Vec<String>, ParameterError> {
        let res = self
            .client
            .get_parameters()
            .set_names(Some(
                names.iter().map(|s| s.to_string()).collect::<Vec<String>>(),
            ))
            .with_decryption(true)
            .send()
            .await
            .with_context(|| "failed to get parameters")
            .map_err(ParameterError::Unknown)?;

        if res.parameters().is_empty() {
            return Err(ParameterError::NotFound());
        }
        let values = res
            .parameters()
            .iter()
            .map(|p| p.value().ok_or(ParameterError::NotFound()));

        values
            .into_iter()
            .map(|p| p.map(|v| v.to_string()))
            .collect()
    }
}
