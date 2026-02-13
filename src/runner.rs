use anyhow::{Context, Result};

use crate::state::{now_rfc3339, RunContext, StepStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyStatus {
    Complete,
    Incomplete,
}

pub trait Step {
    fn id(&self) -> &'static str;
    fn description(&self) -> &'static str;

    fn preflight(&self, ctx: &mut RunContext) -> Result<()>;
    fn apply(&self, ctx: &mut RunContext) -> Result<()>;
    fn verify(&self, ctx: &mut RunContext) -> Result<VerifyStatus>;

    fn undo(&self, _ctx: &mut RunContext) -> Result<()> {
        Ok(())
    }
}

pub struct Runner {
    steps: Vec<Box<dyn Step>>,
}

impl Runner {
    pub fn new(steps: Vec<Box<dyn Step>>) -> Self {
        Self { steps }
    }

    pub fn run(&self, ctx: &mut RunContext) -> Result<()> {
        ctx.state.dry_run = ctx.dry_run;
        ctx.persist()?;

        for step in &self.steps {
            let step_id = step.id();
            let step_name = step.description();
            println!("==> {} ({})", step_name, step_id);

            let index = ctx.state.ensure_step(step_id);
            {
                let record = &mut ctx.state.steps[index];
                record.status = StepStatus::Running;
                record.started_at = Some(now_rfc3339());
                record.finished_at = None;
                record.error = None;
                record.skipped_apply = false;
            }
            ctx.persist()?;

            let result = (|| -> Result<()> {
                step.preflight(ctx)
                    .with_context(|| format!("Preflight failed for step {step_id}"))?;

                if let VerifyStatus::Complete = step
                    .verify(ctx)
                    .with_context(|| format!("Verify failed for step {step_id}"))?
                {
                    let record = &mut ctx.state.steps[index];
                    record.status = StepStatus::Complete;
                    record.finished_at = Some(now_rfc3339());
                    record.skipped_apply = true;
                    ctx.persist()?;
                    println!("    already complete");
                    return Ok(());
                }

                if ctx.dry_run {
                    let record = &mut ctx.state.steps[index];
                    record.status = StepStatus::DryRun;
                    record.finished_at = Some(now_rfc3339());
                    record.skipped_apply = true;
                    ctx.persist()?;
                    println!("    dry-run: apply skipped");
                    return Ok(());
                }

                step.apply(ctx)
                    .with_context(|| format!("Apply failed for step {step_id}"))?;

                match step
                    .verify(ctx)
                    .with_context(|| format!("Verify failed for step {step_id}"))?
                {
                    VerifyStatus::Complete => {
                        let record = &mut ctx.state.steps[index];
                        record.status = StepStatus::Complete;
                        record.finished_at = Some(now_rfc3339());
                        record.skipped_apply = false;
                        ctx.persist()?;
                        Ok(())
                    }
                    VerifyStatus::Incomplete => anyhow::bail!(
                        "Step {step_id} did not verify after apply. See logs/state for details."
                    ),
                }
            })();

            if let Err(err) = result {
                let record = &mut ctx.state.steps[index];
                record.status = StepStatus::Failed;
                record.finished_at = Some(now_rfc3339());
                record.error = Some(err.to_string());
                ctx.persist()?;
                return Err(err);
            }
        }

        Ok(())
    }
}
