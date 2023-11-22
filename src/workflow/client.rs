use super::{Status, Wave, WaveStatus, Workflow};
use crate::aws::{config, to_attribute_value, DynamodbClient};
use anyhow::Context;
use chrono::Utc;
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
                updated_at: None,
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
                    .expression_attribute_values(
                        ":id",
                        to_attribute_value(owner + "/" + &repo).unwrap(),
                    ),
            )
            .await
            .with_context(|| "list workflows")
    }

    pub(crate) async fn get_to_process(&self) -> Result<Vec<Workflow>, anyhow::Error> {
        todo!()
    }

    pub(crate) async fn mark_workflow_done(
        &self,
        w: Workflow,
        status: Status,
    ) -> Result<(), anyhow::Error> {
        self.table
            .run_update::<Workflow>(
                self.table
                    .update()
                    .key("id", to_attribute_value(w.id).unwrap())
                    .key(
                        "created_at",
                        to_attribute_value(w.created_at.to_rfc3339()).unwrap(),
                    )
                    .update_expression("SET #status = :status, #updated_at = :updated_at")
                    .condition_expression("attribute_exists(id) and attribute_exists(created_at)")
                    .expression_attribute_names("#status", "status")
                    .expression_attribute_names("#updated_at", "updated_at")
                    .expression_attribute_values(":status", to_attribute_value(status).unwrap())
                    .expression_attribute_values(
                        ":updated_at",
                        to_attribute_value(Utc::now()).unwrap(),
                    ),
            )
            .await?;

        Ok(())
    }

    pub(crate) async fn update_waves(
        &self,
        w: Workflow,
        waves: Vec<Wave>,
    ) -> Result<(), anyhow::Error> {
        self.table
            .run_update::<Workflow>(
                self.table
                    .update()
                    .key("id", to_attribute_value(w.id).unwrap())
                    .key(
                        "created_at",
                        to_attribute_value(w.created_at.to_rfc3339()).unwrap(),
                    )
                    .update_expression("SET #waves = :waves, #updated_at = :updated_at")
                    .condition_expression("attribute_exists(id) and attribute_exists(created_at)")
                    .expression_attribute_names("#waves", "waves")
                    .expression_attribute_names("#updated_at", "updated_at")
                    .expression_attribute_values(":waves", to_attribute_value(waves).unwrap())
                    .expression_attribute_values(
                        ":updated_at",
                        to_attribute_value(Utc::now()).unwrap(),
                    ),
            )
            .await?;

        Ok(())
    }
}

pub async fn client() -> &'static Client {
    static CONFIG: OnceCell<Client> = OnceCell::const_new();
    CONFIG.get_or_init(Client::new).await
}
