use anyhow::{Context, Result};
use std::io::ErrorKind;
use std::process::Command;

use crate::runner::{Step, VerifyStatus};
use crate::state::RunContext;

pub struct PreflightStep {
    required: Vec<RequiredCommand>,
}

impl PreflightStep {
    pub fn new() -> Self {
        Self {
            required: vec![
                RequiredCommand::new("git", &["--version"], "git"),
                RequiredCommand::new("brew", &["--version"], "homebrew"),
                RequiredCommand::new("gh", &["--version"], "GitHub CLI"),
            ],
        }
    }

    fn check_required(&self) -> Result<()> {
        let mut missing = Vec::new();
        let mut failures = Vec::new();

        for cmd in &self.required {
            match check_command(cmd.name, cmd.args) {
                Ok(()) => {}
                Err(err) => {
                    let not_found = err.chain().any(|cause| {
                        cause
                            .downcast_ref::<std::io::Error>()
                            .map_or(false, |io_err| io_err.kind() == ErrorKind::NotFound)
                    });

                    if not_found {
                        missing.push(cmd.label);
                    } else {
                        failures.push(format!("{}: {}", cmd.label, err));
                    }
                }
            }
        }

        if !missing.is_empty() {
            anyhow::bail!("Missing required tools: {}", missing.join(", "));
        }

        if !failures.is_empty() {
            anyhow::bail!(
                "Required tools failed to run: {}",
                failures.join("; ")
            );
        }

        Ok(())
    }
}

impl Default for PreflightStep {
    fn default() -> Self {
        Self::new()
    }
}

impl Step for PreflightStep {
    fn id(&self) -> &'static str {
        "preflight"
    }

    fn description(&self) -> &'static str {
        "Preflight checks"
    }

    fn preflight(&self, _ctx: &mut RunContext) -> Result<()> {
        self.check_required().context("preflight checks failed")
    }

    fn apply(&self, _ctx: &mut RunContext) -> Result<()> {
        Ok(())
    }

    fn verify(&self, _ctx: &mut RunContext) -> Result<VerifyStatus> {
        self.check_required()?;
        Ok(VerifyStatus::Complete)
    }
}

struct RequiredCommand {
    name: &'static str,
    args: &'static [&'static str],
    label: &'static str,
}

impl RequiredCommand {
    fn new(name: &'static str, args: &'static [&'static str], label: &'static str) -> Self {
        Self { name, args, label }
    }
}

fn check_command(name: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(name).args(args).output();

    match output {
        Ok(result) if result.status.success() => Ok(()),
        Ok(result) => anyhow::bail!(
            "{} returned non-zero status: {:?}",
            name,
            result.status.code()
        ),
        Err(err) => Err(err).context(format!("failed to execute {name}")),
    }
}
