use super::{Status, Wave, WaveStatus, Workflow};
use crate::aws::{config, AttributeValue, DynamodbClient};
use anyhow::Context;
use tokio::sync::OnceCell;

pub struct Client {
    table: DynamodbClient,
}

impl Client {
    async fn new() -> Client {
        let table_name = std::env::var("DYNAMODB_WORKFLOWS")
            .expect("DYNAMODB_WORKFLOWS is required but not set");

        Client {
            table: DynamodbClient::new(config().await, table_name),
        }
    }

    pub async fn create(
        &self,
        workflow: super::CreateWorkflowRequest,
    ) -> Result<(), anyhow::Error> {
        let waves = workflow
            .waves
            .into_iter()
            .map(|w| Wave {
                name: w,
                status: WaveStatus::Pending,
                started_at: None,
                finished_at: None,
            })
            .collect::<Vec<_>>();
        self.table
            .put_item(Workflow {
                id: workflow.owner.clone() + "/" + &workflow.repo,
                created_at: chrono::Utc::now(),
                github_token: workflow.github_token.clone(),
                git_ref: workflow.git_ref.clone(),
                owner: workflow.owner.clone(),
                repo: workflow.repo.clone(),
                sha: workflow.sha.clone(),
                stability_period_minutes: workflow.stability_period_minutes,
                waves,
                workflow: workflow.workflow.clone(),
                status: Status::Running,
                commit_message: workflow.commit_message.clone(),
            })
            .await
            .with_context(|| "create workflow")
    }

    pub async fn list(&self, owner: String, repo: String) -> Result<Vec<Workflow>, anyhow::Error> {
        self.table
            .run_query::<Workflow>(
                self.table
                    .query()
                    .key_condition_expression("#id = :id")
                    .expression_attribute_names("#id", "id")
                    .expression_attribute_values(":id", AttributeValue::S(owner + "/" + &repo)),
            )
            .await
            .with_context(|| "list workflows")
    }
}

pub async fn client() -> &'static Client {
    static CONFIG: OnceCell<Client> = OnceCell::const_new();
    CONFIG.get_or_init(Client::new).await
}
