use super::{CreatedAt, Environment, EnvironmentStatus, Status, Workflow};
use crate::aws::{config, to_attribute_value, DynamodbClient};
use anyhow::Context;
use chrono::{DateTime, Utc};
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
        let environments = workflow
            .environments
            .into_iter()
            .map(|w| Environment {
                name: w,
                status: EnvironmentStatus::Pending,
                started_at: None,
                finished_at: None,
                deployment_id: None,
            })
            .collect::<Vec<_>>();
        self.table
            .put_item(Workflow {
                id: workflow.owner.clone() + "/" + &workflow.repo,
                created_at: CreatedAt::now(),
                git_ref: workflow.git_ref.clone(),
                owner: workflow.owner.clone(),
                repo: workflow.repo.clone(),
                sha: workflow.sha.clone(),
                stability_period_minutes: workflow.stability_period_minutes,
                environments,
                status: Status::Running,
                commit_message: workflow.commit_message.clone(),
                updated_at: None,
                due_to_run: Utc::now(),
            })
            .await
            .context("create workflow")
    }

    pub async fn list(&self, owner: String, repo: String) -> Result<Vec<Workflow>, anyhow::Error> {
        self.table
            .run_query(
                self.table
                    .query()
                    .key_condition_expression("#id = :id")
                    .expression_attribute_names("#id", "id")
                    .expression_attribute_values(":id", to_attribute_value(owner + "/" + &repo)?),
            )
            .await
            .context("list workflows")
    }

    pub(crate) async fn get_due_to_run(
        &self,
        due_to_run: DateTime<Utc>,
    ) -> Result<Vec<Workflow>, anyhow::Error> {
        self.table
            .run_query(
                self.table
                    .query()
                    .index_name("workflows_by_status")
                    .key_condition_expression("#status = :status and #due_to_run <= :due_to_run")
                    .expression_attribute_names("#status", "status")
                    .expression_attribute_values(
                        ":status",
                        to_attribute_value(Status::Running.to_string())?,
                    )
                    .expression_attribute_names("#due_to_run", "due_to_run")
                    .expression_attribute_values(
                        ":due_to_run",
                        to_attribute_value(due_to_run.to_rfc3339())?,
                    ),
            )
            .await
            .context("get due to run")
    }

    pub(crate) async fn fail_environment(
        &self,
        w: Workflow,
        environments: Vec<Environment>,
        due_to_run: DateTime<Utc>,
    ) -> Result<Workflow, anyhow::Error> {
        self.table
            .run_update(
                self.table
                    .update()
                    .key("id", to_attribute_value(w.id)?)
                    .key("created_at", to_attribute_value(w.created_at.to_rfc3339())?)
                    .update_expression("SET #environments = :environments, #due_to_run = :due_to_run, #updated_at = :updated_at, #status = :status")
                    .condition_expression("attribute_exists(#id) and attribute_exists(#created_at)")
                    .expression_attribute_names("#environments", "environments")
                    .expression_attribute_names("#due_to_run", "due_to_run")
                    .expression_attribute_names("#updated_at", "updated_at")
                    .expression_attribute_names("#id", "id")
                    .expression_attribute_names("#created_at", "created_at")
                    .expression_attribute_names("#status", "status")
                    .expression_attribute_values(":due_to_run", to_attribute_value(due_to_run)?)
                    .expression_attribute_values(":environments", to_attribute_value(environments)?)
                    .expression_attribute_values(":updated_at", to_attribute_value(Utc::now())?)
                    .expression_attribute_values(":status", to_attribute_value(Status::Failure)?),
            )
            .await
            .context("failing environment")
    }

    pub(crate) async fn complete_environment(
        &self,
        w: Workflow,
        environments: Vec<Environment>,
        due_to_run: DateTime<Utc>,
    ) -> Result<Workflow, anyhow::Error> {
        self.table
            .run_update(
                self.table
                    .update()
                    .key("id", to_attribute_value(w.id)?)
                    .key("created_at", to_attribute_value(w.created_at.to_rfc3339())?)
                    .update_expression("SET #environments = :environments, #due_to_run = :due_to_run, #updated_at = :updated_at")
                    .condition_expression("attribute_exists(#id) and attribute_exists(#created_at)")
                    .expression_attribute_names("#environments", "environments")
                    .expression_attribute_names("#due_to_run", "due_to_run")
                    .expression_attribute_names("#updated_at", "updated_at")
                    .expression_attribute_names("#id", "id")
                    .expression_attribute_names("#created_at", "created_at")
                    .expression_attribute_values(":due_to_run", to_attribute_value(due_to_run)?)
                    .expression_attribute_values(":environments", to_attribute_value(environments)?)
                    .expression_attribute_values(":updated_at", to_attribute_value(Utc::now())?),
            )
            .await
            .context("completing environment")
    }

    pub(crate) async fn mark_workflow_done(
        &self,
        w: Workflow,
        status: Status,
    ) -> Result<(), anyhow::Error> {
        self.table
            .run_update(
                self.table
                    .update()
                    .key("id", to_attribute_value(w.id)?)
                    .key("created_at", to_attribute_value(w.created_at.to_rfc3339())?)
                    .update_expression("SET #status = :status, #updated_at = :updated_at")
                    .condition_expression("attribute_exists(#id) and attribute_exists(#created_at)")
                    .expression_attribute_names("#status", "status")
                    .expression_attribute_names("#updated_at", "updated_at")
                    .expression_attribute_names("#id", "id")
                    .expression_attribute_names("#created_at", "created_at")
                    .expression_attribute_values(":status", to_attribute_value(status)?)
                    .expression_attribute_values(":updated_at", to_attribute_value(Utc::now())?),
            )
            .await
            .context("marking workflow done")?;

        Ok(())
    }

    pub(crate) async fn update_environments(
        &self,
        w: Workflow,
        environments: Vec<Environment>,
    ) -> Result<Workflow, anyhow::Error> {
        log::info!(
            "updating environments for workflow {}, {}",
            w.id,
            w.created_at.to_rfc3339()
        );

        Ok(self
            .table
            .run_update(
                self.table
                    .update()
                    .key("id", to_attribute_value(w.id)?)
                    .key("created_at", to_attribute_value(w.created_at.to_rfc3339())?)
                    .update_expression(
                        "SET #environments = :environments, #updated_at = :updated_at",
                    )
                    .condition_expression("attribute_exists(#id) and attribute_exists(#created_at)")
                    .expression_attribute_names("#environments", "environments")
                    .expression_attribute_names("#updated_at", "updated_at")
                    .expression_attribute_names("#id", "id")
                    .expression_attribute_names("#created_at", "created_at")
                    .expression_attribute_values(":environments", to_attribute_value(environments)?)
                    .expression_attribute_values(":updated_at", to_attribute_value(Utc::now())?),
            )
            .await
            .context("updating environments")?)
    }
}

pub async fn client() -> &'static Client {
    static CONFIG: OnceCell<Client> = OnceCell::const_new();
    CONFIG.get_or_init(Client::new).await
}
