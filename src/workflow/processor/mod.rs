use crate::github;

use super::EnvironmentStatus;
use anyhow::Context;
use chrono::Utc;

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

impl From<github::WorkflowStatus> for EnvironmentStatus {
    fn from(status: github::WorkflowStatus) -> Self {
        match status {
            github::WorkflowStatus::Completed => EnvironmentStatus::Success,
            github::WorkflowStatus::ActionRequired => EnvironmentStatus::Failure,
            github::WorkflowStatus::Cancelled => EnvironmentStatus::Failure,
            github::WorkflowStatus::Failure => EnvironmentStatus::Failure,
            github::WorkflowStatus::Neutral => EnvironmentStatus::Failure,
            github::WorkflowStatus::Skipped => EnvironmentStatus::Failure,
            github::WorkflowStatus::Stale => EnvironmentStatus::Failure,
            github::WorkflowStatus::Success => EnvironmentStatus::Success,
            github::WorkflowStatus::TimedOut => EnvironmentStatus::Failure,
            github::WorkflowStatus::InProgress => EnvironmentStatus::Running,
            github::WorkflowStatus::Queued => EnvironmentStatus::Queued,
            github::WorkflowStatus::Requested => EnvironmentStatus::Queued,
            github::WorkflowStatus::Waiting => EnvironmentStatus::Queued,
            github::WorkflowStatus::Pending => EnvironmentStatus::Queued,
        }
    }
}

fn overall_status(workflows: Vec<github::Workflow>) -> EnvironmentStatus {
    workflows
        .into_iter()
        .map(|w| w.status.into())
        .min()
        .unwrap_or(EnvironmentStatus::Running)
}

async fn process_workflow(
    client: &'static super::Client,
    workflow: super::Workflow,
) -> Result<(), anyhow::Error> {
    match workflow.next_environment() {
        None => {
            // Nothing left to do, mark the workflow as done. To do this,
            // find the last environment with a status of Success or Failure, and use that as the status.
            let w = workflow
                .environments
                .iter()
                .rev()
                .find(|w| w.status.is_terminal());
            let status = w.map(|w| w.status).unwrap_or(EnvironmentStatus::Success);
            client.mark_workflow_done(workflow, status.into()).await
        }
        Some((idx, w)) => {
            if w.status == EnvironmentStatus::Running {
                // it's running, we need to check the status of the workflows.
                let github_workflows = github::list_workflows(
                    &workflow.owner,
                    &workflow.repo,
                    &workflow.sha,
                    "deployment",
                )
                .await
                .context("listing github workflows")?;

                log::info!(
                    "found workflows {:?} for commit sha {}",
                    github_workflows,
                    &workflow.sha
                );

                // If there are no workflows a couple of minutes after it triggered, then
                // yolo it as done
                if let Some(started_at) = w.started_at {
                    if github_workflows.is_empty()
                        && started_at + chrono::Duration::minutes(5) < Utc::now()
                    {
                        log::info!(
                            "no workflows found for commit sha {} after 5 minutes, marking as done",
                            &workflow.sha
                        );
                        let mut environments = workflow.environments.clone();
                        if let Some(environment) = environments.get_mut(idx) {
                            environment.status = EnvironmentStatus::Success;
                            environment.finished_at = Some(Utc::now());
                        }
                        let next_due_to_run = Utc::now()
                            + chrono::Duration::minutes(workflow.stability_period_minutes as i64);

                        client
                            .complete_environment(workflow, environments, next_due_to_run)
                            .await
                            .context("completing environment")?;
                        return Ok(());
                    }
                }

                let status = overall_status(github_workflows);
                log::info!("step is {:?} for commit sha {}", status, &workflow.sha);

                let mut environments = workflow.environments.clone();
                if let Some(environment) = environments.get_mut(idx) {
                    environment.status = status;

                    let next_due_to_run = if status.is_terminal() {
                        environment.finished_at = Some(Utc::now());
                        let next_due_to_run = Utc::now()
                            + chrono::Duration::minutes(workflow.stability_period_minutes as i64);
                        log::info!(
                            "environment {} finished, marking workflow as done, and next due at {:?}",
                            w.name,
                            next_due_to_run
                        );

                        next_due_to_run
                    } else {
                        workflow.due_to_run
                    };

                    let deployment_id = environment.deployment_id.unwrap();
                    let workflow = match status {
                        EnvironmentStatus::Success => client
                            .complete_environment(workflow, environments, next_due_to_run)
                            .await
                            .context("completing environment")?,
                        EnvironmentStatus::Failure => client
                            .fail_environment(workflow, environments, next_due_to_run)
                            .await
                            .context("failing environment")?,
                        EnvironmentStatus::Running | EnvironmentStatus::Queued => client
                            .update_environments(workflow, environments)
                            .await
                            .context("updating step status")?,
                        _ => unreachable!(),
                    };

                    github::update_deployment_status(
                        &workflow.owner,
                        &workflow.repo,
                        &deployment_id,
                        status.into(),
                    )
                    .await
                    .context("updating deployment status")?;
                }

                return Ok(());
            };

            log::info!("picked up environment {} to process", w.name);

            let deployment = github::create_deployment(github::CreateDeploymentRequest {
                owner: &workflow.owner,
                repo: &workflow.repo,
                environment: &w.name,
                git_ref: &workflow.sha,
                description: "created by pipedream",
            })
            .await
            .context("running github workflow")?;

            log::info!("environment {} started", w.name);

            let environment_name = w.name.clone();
            let mut environments = workflow.environments.clone();
            if let Some(environment) = environments.get_mut(idx) {
                environment.status = EnvironmentStatus::Running;
                environment.started_at = Some(Utc::now());
                environment.deployment_id = Some(deployment.id);
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
