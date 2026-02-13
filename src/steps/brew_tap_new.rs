use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::runner::{Step, VerifyStatus};
use crate::state::RunContext;

pub struct BrewTapNewStep;

impl BrewTapNewStep {
    pub fn new() -> Self {
        Self
    }

    fn ensure_tap_path(ctx: &mut RunContext) -> Result<PathBuf> {
        if let Some(path) = ctx.state.tap_path.as_deref() {
            return Ok(PathBuf::from(path));
        }

        let output = Command::new("brew")
            .arg("--repository")
            .output()
            .context("failed to run brew --repository")?;

        if !output.status.success() {
            anyhow::bail!(
                "brew --repository returned non-zero status: {:?}",
                output.status.code()
            );
        }

        let base = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if base.is_empty() {
            anyhow::bail!("brew --repository returned empty output");
        }

        let tap_path = PathBuf::from(base)
            .join("Library")
            .join("Taps")
            .join(&ctx.inputs.owner)
            .join(&ctx.inputs.repo_name);

        ctx.state.tap_path = Some(tap_path.to_string_lossy().to_string());
        ctx.persist()?;

        Ok(tap_path)
    }
}

impl Default for BrewTapNewStep {
    fn default() -> Self {
        Self::new()
    }
}

impl Step for BrewTapNewStep {
    fn id(&self) -> &'static str {
        "brew_tap_new"
    }

    fn description(&self) -> &'static str {
        "Create local tap (brew tap-new)"
    }

    fn preflight(&self, _ctx: &mut RunContext) -> Result<()> {
        Ok(())
    }

    fn apply(&self, ctx: &mut RunContext) -> Result<()> {
        let repo_slug = ctx.inputs.repo_slug();
        println!("    brew tap-new {}", repo_slug);

        let status = Command::new("brew")
            .arg("tap-new")
            .arg(repo_slug)
            .status()
            .context("failed to run brew tap-new")?;

        if !status.success() {
            anyhow::bail!("brew tap-new returned non-zero status: {:?}", status.code());
        }

        let _ = Self::ensure_tap_path(ctx)?;
        Ok(())
    }

    fn verify(&self, ctx: &mut RunContext) -> Result<VerifyStatus> {
        let tap_path = Self::ensure_tap_path(ctx)?;

        if !tap_path.exists() {
            return Ok(VerifyStatus::Incomplete);
        }

        let git_dir = tap_path.join(".git");
        if !git_dir.is_dir() {
            anyhow::bail!(
                "tap path exists but is not a git repo: {}",
                tap_path.display()
            );
        }

        Ok(VerifyStatus::Complete)
    }
}
