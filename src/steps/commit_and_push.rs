use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::runner::{Step, VerifyStatus};
use crate::state::RunContext;

pub struct CommitAndPushStep;

impl CommitAndPushStep {
    pub fn new() -> Self {
        Self
    }

    fn tap_path<'a>(ctx: &'a RunContext) -> Result<&'a str> {
        ctx.state
            .tap_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("tap path is not set; brew tap-new must run first"))
    }

    fn ensure_origin(path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args(["-C", path.to_str().unwrap_or(""), "remote", "get-url", "origin"])
            .output()
            .context("failed to read git remote origin")?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("origin remote is missing: {}", stderr.trim());
    }

    fn status_info(path: &Path) -> Result<StatusInfo> {
        let porcelain = Command::new("git")
            .args(["-C", path.to_str().unwrap_or(""), "status", "--porcelain"])
            .output()
            .context("failed to run git status --porcelain")?;

        if !porcelain.status.success() {
            let stderr = String::from_utf8_lossy(&porcelain.stderr);
            anyhow::bail!("git status --porcelain failed: {}", stderr.trim());
        }

        let dirty = !String::from_utf8_lossy(&porcelain.stdout).trim().is_empty();

        let short = Command::new("git")
            .args(["-C", path.to_str().unwrap_or(""), "status", "-sb"])
            .output()
            .context("failed to run git status -sb")?;

        if !short.status.success() {
            let stderr = String::from_utf8_lossy(&short.stderr);
            anyhow::bail!("git status -sb failed: {}", stderr.trim());
        }

        let output = String::from_utf8_lossy(&short.stdout);
        let first_line = output.lines().next().unwrap_or("").trim();
        let mut branch = "".to_string();
        let mut has_upstream = false;
        let mut ahead = 0usize;
        let mut behind = 0usize;

        if let Some(line) = first_line.strip_prefix("## ") {
            if let Some((branch_part, rest)) = line.split_once("...") {
                branch = branch_part.trim().to_string();
                has_upstream = true;

                if let Some(start) = rest.find('[') {
                    if let Some(end) = rest[start + 1..].find(']') {
                        let inside = &rest[start + 1..start + 1 + end];
                        for part in inside.split(',') {
                            let part = part.trim();
                            if let Some(value) = part.strip_prefix("ahead ") {
                                ahead = value.trim().parse().unwrap_or(0);
                            } else if let Some(value) = part.strip_prefix("behind ") {
                                behind = value.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                }
            } else {
                branch = line.trim().to_string();
                has_upstream = false;
            }
        }

        if branch.is_empty() {
            let rev = Command::new("git")
                .args(["-C", path.to_str().unwrap_or(""), "rev-parse", "--abbrev-ref", "HEAD"])
                .output()
                .context("failed to read current branch")?;

            if !rev.status.success() {
                let stderr = String::from_utf8_lossy(&rev.stderr);
                anyhow::bail!("git rev-parse failed: {}", stderr.trim());
            }

            branch = String::from_utf8_lossy(&rev.stdout).trim().to_string();
        }

        Ok(StatusInfo {
            dirty,
            ahead,
            behind,
            has_upstream,
            branch,
        })
    }

    fn commit_changes(path: &Path, message: &str) -> Result<()> {
        let status = Command::new("git")
            .args(["-C", path.to_str().unwrap_or(""), "add", "-A"])
            .status()
            .context("failed to stage changes")?;

        if !status.success() {
            anyhow::bail!("git add returned non-zero status: {:?}", status.code());
        }

        let output = Command::new("git")
            .args(["-C", path.to_str().unwrap_or(""), "commit", "-m", message])
            .output()
            .context("failed to commit changes")?;

        if output.status.success() {
            return Ok(());
        }

        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_lowercase();

        if combined.contains("nothing to commit") {
            return Ok(());
        }

        anyhow::bail!("git commit failed: {}", combined.trim());
    }

    fn push_changes(path: &Path, branch: &str, set_upstream: bool) -> Result<()> {
        let mut args = vec!["-C", path.to_str().unwrap_or(""), "push"];
        if set_upstream {
            args.push("-u");
            args.push("origin");
            args.push(branch);
        }

        let status = Command::new("git")
            .args(args)
            .status()
            .context("failed to push changes")?;

        if !status.success() {
            anyhow::bail!("git push returned non-zero status: {:?}", status.code());
        }

        Ok(())
    }
}

impl Default for CommitAndPushStep {
    fn default() -> Self {
        Self::new()
    }
}

impl Step for CommitAndPushStep {
    fn id(&self) -> &'static str {
        "commit_and_push"
    }

    fn description(&self) -> &'static str {
        "Commit and push changes"
    }

    fn preflight(&self, ctx: &mut RunContext) -> Result<()> {
        let tap_path = Self::tap_path(ctx)?;
        let path = Path::new(tap_path);

        if !path.exists() {
            anyhow::bail!("tap path does not exist: {}", path.display());
        }

        if !path.join(".git").is_dir() {
            anyhow::bail!("tap path is not a git repo: {}", path.display());
        }

        Self::ensure_origin(path)?;
        Ok(())
    }

    fn apply(&self, ctx: &mut RunContext) -> Result<()> {
        let tap_path = Self::tap_path(ctx)?;
        let path = Path::new(tap_path);

        let mut status = Self::status_info(path)?;
        if status.behind > 0 {
            anyhow::bail!("local branch is behind origin; pull is required before pushing");
        }

        if status.dirty {
            Self::commit_changes(path, "Update tap files")?;
        }

        status = Self::status_info(path)?;
        if status.behind > 0 {
            anyhow::bail!("local branch is behind origin; pull is required before pushing");
        }

        if status.ahead > 0 || !status.has_upstream {
            Self::push_changes(path, &status.branch, !status.has_upstream)?;
        }

        Ok(())
    }

    fn verify(&self, ctx: &mut RunContext) -> Result<VerifyStatus> {
        let tap_path = Self::tap_path(ctx)?;
        let path = Path::new(tap_path);

        let status = Self::status_info(path)?;
        if status.behind > 0 {
            anyhow::bail!("local branch is behind origin; pull is required before pushing");
        }

        if status.dirty || status.ahead > 0 || !status.has_upstream {
            return Ok(VerifyStatus::Incomplete);
        }

        Ok(VerifyStatus::Complete)
    }
}

struct StatusInfo {
    dirty: bool,
    ahead: usize,
    behind: usize,
    has_upstream: bool,
    branch: String,
}
