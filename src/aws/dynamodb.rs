#![allow(dead_code)]
use std::collections::HashMap;

use anyhow::{Context, Error};
use aws_sdk_dynamodb::operation::query::builders::QueryFluentBuilder;
use aws_sdk_dynamodb::operation::update_item::builders::UpdateItemFluentBuilder;
use aws_sdk_dynamodb::types::ReturnValue;
use aws_sdk_dynamodb::Client;
use aws_types::sdk_config;
use serde::{Deserialize, Serialize};
use serde_dynamo::{from_item, from_items, to_attribute_value, to_item};

pub use aws_sdk_dynamodb::types::AttributeValue;

#[derive(Clone)]
pub struct DynamodbClient {
    client: Client,
    table_name: String,
}

#[derive(thiserror::Error, Debug)]
pub enum DynamodbError {
    #[error("The requested item could not be found")]
    NotFound(),
    #[error("An unexpected error occurred: {0:#}")]
    Unexpected(Error),
}

impl From<anyhow::Error> for DynamodbError {
    fn from(e: anyhow::Error) -> Self {
        DynamodbError::Unexpected(e)
    }
}

impl DynamodbClient {
    pub fn new(config: &sdk_config::SdkConfig, table_name: String) -> DynamodbClient {
        DynamodbClient {
            client: Client::new(config),
            table_name,
        }
    }

    pub async fn put_item<T: Serialize>(&self, item: T) -> Result<(), DynamodbError> {
        let db_item = to_item(item).with_context(|| "failed to convert item to dynamodb item")?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(db_item))
            .send()
            .await
            .with_context(|| "failed to put item")?;

        Ok(())
    }

    pub async fn get_item<'a, T: Deserialize<'a>>(
        &self,
        key: HashMap<String, impl Serialize>,
    ) -> Result<T, DynamodbError> {
        let mut keys = HashMap::new();
        for (k, v) in key.into_iter() {
            let value = to_attribute_value(v)
                .with_context(|| format!("failed to marshal key to attribute value {}", k))?;
            keys.insert(k, value);
        }

        let res = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .set_key(Some(keys))
            .send()
            .await
            .with_context(|| "failed to get item")?;

        let item = res.item.ok_or(DynamodbError::NotFound())?;
        let item = from_item(item).with_context(|| "failed to deserialize item")?;

        Ok(item)
    }

    pub fn query(&self) -> QueryFluentBuilder {
        self.client.query().table_name(&self.table_name)
    }

    pub async fn run_query<'a, T: Deserialize<'a>>(
        &self,
        query: QueryFluentBuilder,
    ) -> Result<Vec<T>, DynamodbError> {
        let res = query.send().await.with_context(|| "failed to query")?;

        match res.items {
            None => Ok(vec![]),
            Some(items) => {
                let items: Vec<T> =
                    from_items(items).with_context(|| "failed to deserialize results")?;
                Ok(items)
            }
        }
    }

    pub fn update(&self) -> UpdateItemFluentBuilder {
        self.client
            .update_item()
            .table_name(&self.table_name)
            .return_values(ReturnValue::AllNew)
    }

    pub async fn run_update<'a, T: Deserialize<'a>>(
        &self,
        update: UpdateItemFluentBuilder,
    ) -> Result<T, DynamodbError> {
        let res = update.send().await.with_context(|| "failed to update")?;

        match res.attributes {
            None => Err(DynamodbError::NotFound()),
            Some(attributes) => {
                let item = from_item(attributes).with_context(|| "failed to deserialize item")?;
                Ok(item)
            }
        }
    }
}
