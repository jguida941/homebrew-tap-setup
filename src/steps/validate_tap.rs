use anyhow::{Context, Result};
use std::process::Command;

use crate::runner::{Step, VerifyStatus};
use crate::state::RunContext;

pub struct ValidateTapStep;

impl ValidateTapStep {
    pub fn new() -> Self {
        Self
    }

    fn tap_candidates(ctx: &RunContext) -> Vec<String> {
        let mut candidates = vec![ctx.inputs.repo_slug()];
        let shorthand = format!("{}/{}", ctx.inputs.owner, ctx.inputs.tap);
        let expected_repo = format!("homebrew-{}", ctx.inputs.tap);

        if ctx.inputs.repo_name == expected_repo {
            candidates.push(shorthand);
        }

        candidates
    }

    fn preferred_tap(ctx: &RunContext) -> String {
        let shorthand = format!("{}/{}", ctx.inputs.owner, ctx.inputs.tap);
        let expected_repo = format!("homebrew-{}", ctx.inputs.tap);

        if ctx.inputs.repo_name == expected_repo {
            shorthand
        } else {
            ctx.inputs.repo_slug()
        }
    }

    fn is_tapped(identifier: &str) -> Result<bool> {
        let output = Command::new("brew")
            .arg("tap")
            .output()
            .context("failed to run brew tap")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("brew tap returned non-zero status: {}", stderr.trim());
        }

        let list = String::from_utf8_lossy(&output.stdout);
        Ok(list.lines().any(|line| line.trim() == identifier))
    }
}

impl Default for ValidateTapStep {
    fn default() -> Self {
        Self::new()
    }
}

impl Step for ValidateTapStep {
    fn id(&self) -> &'static str {
        "validate_tap"
    }

    fn description(&self) -> &'static str {
        "Validate tap is registered"
    }

    fn preflight(&self, _ctx: &mut RunContext) -> Result<()> {
        Ok(())
    }

    fn apply(&self, ctx: &mut RunContext) -> Result<()> {
        let identifier = Self::preferred_tap(ctx);
        println!("    brew tap {}", identifier);

        let status = Command::new("brew")
            .args(["tap", &identifier])
            .status()
            .context("failed to run brew tap")?;

        if !status.success() {
            anyhow::bail!("brew tap returned non-zero status: {:?}", status.code());
        }

        Ok(())
    }

    fn verify(&self, ctx: &mut RunContext) -> Result<VerifyStatus> {
        let candidates = Self::tap_candidates(ctx);
        for identifier in candidates {
            if Self::is_tapped(&identifier)? {
                return Ok(VerifyStatus::Complete);
            }
        }

        Ok(VerifyStatus::Incomplete)
    }
}
