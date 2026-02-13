use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use crate::runner::{Step, VerifyStatus};
use crate::state::RunContext;
use crate::inputs::Visibility;

pub struct GhRepoCreateStep;

impl GhRepoCreateStep {
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

    fn repo_exists(repo_slug: &str) -> Result<bool> {
        let output = Command::new("gh")
            .args(["repo", "view", repo_slug, "--json", "name"])
            .output()
            .context("failed to run gh repo view")?;

        if output.status.success() {
            return Ok(true);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        if is_repo_missing(&stderr) {
            return Ok(false);
        }

        anyhow::bail!("gh repo view failed: {}", stderr.trim())
    }

    fn fetch_repo_urls(repo_slug: &str) -> Result<RepoUrls> {
        let output = Command::new("gh")
            .args([
                "repo",
                "view",
                repo_slug,
                "--json",
                "sshUrl,url",
            ])
            .output()
            .context("failed to run gh repo view")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gh repo view failed: {}", stderr.trim());
        }

        let info: RepoUrls = serde_json::from_slice(&output.stdout)
            .context("failed to parse gh repo view output")?;

        Ok(info)
    }

    fn git_remote_url(path: &Path, remote: &str) -> Result<Option<String>> {
        let output = Command::new("git")
            .args(["-C", path.to_str().unwrap_or(""), "remote", "get-url", remote])
            .output()
            .context("failed to query git remote")?;

        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(Some(url));
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        if stderr.contains("no such remote") || stderr.contains("does not appear to be a git repository") {
            return Ok(None);
        }

        anyhow::bail!("git remote get-url failed: {}", stderr.trim())
    }

    fn ensure_branch(path: &Path, branch: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["-C", path.to_str().unwrap_or(""), "rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .context("failed to read current git branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git rev-parse failed: {}", stderr.trim());
        }

        let current = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if current == branch {
            return Ok(());
        }

        let status = Command::new("git")
            .args(["-C", path.to_str().unwrap_or(""), "branch", "-M", branch])
            .status()
            .context("failed to rename git branch")?;

        if !status.success() {
            anyhow::bail!("git branch -M returned non-zero status: {:?}", status.code());
        }

        Ok(())
    }
}

impl Default for GhRepoCreateStep {
    fn default() -> Self {
        Self::new()
    }
}

impl Step for GhRepoCreateStep {
    fn id(&self) -> &'static str {
        "gh_repo_create"
    }

    fn description(&self) -> &'static str {
        "Create GitHub repo and push"
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

        Ok(())
    }

    fn apply(&self, ctx: &mut RunContext) -> Result<()> {
        let tap_path = Self::tap_path(ctx)?;
        let path = Path::new(tap_path);
        let repo_slug = ctx.inputs.repo_slug();

        Self::ensure_branch(path, &ctx.inputs.branch)?;

        let visibility_flag = match ctx.inputs.visibility {
            Visibility::Public => "--public",
            Visibility::Private => "--private",
        };

        println!("    gh repo create {} --source {} --push", repo_slug, tap_path);

        let status = Command::new("gh")
            .args([
                "repo",
                "create",
                &repo_slug,
                "--source",
                tap_path,
                "--push",
                "--remote",
                "origin",
                visibility_flag,
            ])
            .status()
            .context("failed to run gh repo create")?;

        if !status.success() {
            anyhow::bail!(
                "gh repo create returned non-zero status: {:?}",
                status.code()
            );
        }

        Ok(())
    }

    fn verify(&self, ctx: &mut RunContext) -> Result<VerifyStatus> {
        let tap_path = Self::tap_path(ctx)?;
        let path = Path::new(tap_path);
        let repo_slug = ctx.inputs.repo_slug();

        if !Self::repo_exists(&repo_slug)? {
            return Ok(VerifyStatus::Incomplete);
        }

        let remote_url = match Self::git_remote_url(path, "origin")? {
            Some(url) => url,
            None => {
                anyhow::bail!(
                    "GitHub repo exists but no 'origin' remote is set for {}",
                    path.display()
                );
            }
        };

        let repo_urls = Self::fetch_repo_urls(&repo_slug)?;
        let https_git = format!("{}.git", repo_urls.web_url);
        if remote_url != repo_urls.ssh_url
            && remote_url != repo_urls.web_url
            && remote_url != https_git
        {
            anyhow::bail!(
                "origin remote does not match repo {} (found: {})",
                repo_slug,
                remote_url
            );
        }

        Ok(VerifyStatus::Complete)
    }
}

#[derive(Debug, Deserialize)]
struct RepoUrls {
    #[serde(rename = "sshUrl")]
    ssh_url: String,
    #[serde(rename = "url")]
    web_url: String,
}

fn is_repo_missing(stderr: &str) -> bool {
    let text = stderr.to_lowercase();
    text.contains("not found")
        || text.contains("could not resolve to a repository")
        || text.contains("404")
}
