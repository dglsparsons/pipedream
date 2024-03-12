use super::EnvironmentStatus;
use anyhow::Context;
use chrono::Utc;

mod github;

pub async fn process_workflows(client: &'static super::Client) -> Result<(), anyhow::Error> {
    let workflows = client.get_due_to_run(Utc::now()).await?;

    let futures: Vec<_> = workflows
        .into_iter()
        .map(|w| tokio::spawn(process_workflow(client, w)))
        .collect();

    for f in futures.into_iter() {
        f.await??;
    }
    Ok(())
}

async fn process_workflow(
    client: &'static super::Client,
    workflow: super::Workflow,
) -> Result<(), anyhow::Error> {
    match workflow.next_environment() {
        None => {
            // Nothing to do, mark the workflow as done. To do this,
            // find the last environment with a status of Success or Failure, and use that as the status.
            let w = workflow.environments.iter().rev().find(|w| {
                w.status == EnvironmentStatus::Success || w.status == EnvironmentStatus::Failure
            });
            let status = w.map(|w| w.status).unwrap_or(EnvironmentStatus::Success);
            client.mark_workflow_done(workflow, status.into()).await
        }
        Some((idx, w)) => {
            if w.status == EnvironmentStatus::Running {
                // it's running, so do nothing.
                log::info!("skipping environment {} as it is already deploying", w.name);
                return Ok(());
            };

            log::info!("picked up environment {} to process", w.name);

            github::create_deployment(github::CreateDeploymentRequest {
                owner: &workflow.owner,
                repo: &workflow.repo,
                environment: &w.name,
                git_ref: &workflow.git_ref,
                description: "created by pipedream",
            })
            .await
            .context("running github workflow")?;

            log::info!("environment {} started", w.name);

            let environment_name = w.name.clone();
            let mut environments = workflow.environments.clone();
            if let Some(environment) = environments.get_mut(idx) {
                environment.status = EnvironmentStatus::Running;
            }
            client
                .update_environments(workflow, environments)
                .await
                .context("updating step status")?;

            log::info!(
                "environment {} status updated in database",
                environment_name
            );

            // Then register a webhook to call back to for updating the status
            // and setting the time of the next environment? Or just poll forever.
            Ok(())
        }
    }
}
