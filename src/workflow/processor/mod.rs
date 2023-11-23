use super::WaveStatus;
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
    match workflow.next_wave() {
        None => {
            // Nothing to do, mark the workflow as done. To do this,
            // find the last wave with a status of Success or Failure, and use that as the status.
            let w = workflow
                .waves
                .iter()
                .rev()
                .find(|w| w.status == WaveStatus::Success || w.status == WaveStatus::Failure);
            let status = w.map(|w| w.status).unwrap_or(WaveStatus::Success);
            client.mark_workflow_done(workflow, status.into()).await
        }
        Some((idx, w)) => {
            if w.status == WaveStatus::Running {
                // it's running, so do nothing.
                log::info!("skipping wave {} as it is already running", w.name);
                return Ok(());
            };

            log::info!("picked up wave {} to process", w.name);

            github::run_workflow(github::WorkflowRequest {
                owner: &workflow.owner,
                repo: &workflow.repo,
                workflow: &workflow.workflow,
                wave: &w.name,
                git_ref: &workflow.git_ref,
                sha: &workflow.sha,
                token: &workflow.github_token,
            })
            .await
            .context("running github workflow")?;

            log::info!("wave {} started", w.name);

            let wavename = w.name.clone();
            let mut waves = workflow.waves.clone();
            if let Some(wave) = waves.get_mut(idx) {
                wave.status = WaveStatus::Running;
            }
            client
                .update_waves(workflow, waves)
                .await
                .context("updating step status")?;

            log::info!("wave {} status updated in database", wavename);

            // Then register a webhook to call back to for updating the status
            // and setting the time of the next wave? Or just poll forever.
            Ok(())
        }
    }
}
