use anyhow::{bail, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FormulaMode {
    Stub,
    BrewCreate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inputs {
    pub owner: String,
    pub tap: String,
    pub repo_name: String,
    pub visibility: Visibility,
    pub branch: String,
    pub formula_mode: FormulaMode,
    pub formula_url: Option<String>,
    pub formula_name: Option<String>,
}

impl Inputs {
    pub fn new(
        owner: String,
        tap: String,
        repo_name: Option<String>,
        visibility: Visibility,
        branch: String,
        formula_mode: FormulaMode,
        formula_url: Option<String>,
        formula_name: Option<String>,
    ) -> Result<Self> {
        let owner = normalize_token("owner", owner)?;
        let tap = normalize_token("tap", tap)?;
        let branch = normalize_branch(branch)?;
        let formula_url = formula_url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let formula_name = formula_name
            .map(|value| normalize_token("formula name", value))
            .transpose()?;

        if matches!(formula_mode, FormulaMode::BrewCreate) && formula_url.is_none() {
            bail!("formula-url is required when formula-mode is brew-create");
        }

        if tap.starts_with("homebrew-") {
            eprintln!(
                "Warning: tap short name includes 'homebrew-'; default repo would become 'homebrew-{}'.",
                tap
            );
        }

        let repo_name = match repo_name {
            Some(name) => normalize_token("repo name", name)?,
            None => format!("homebrew-{}", tap),
        };

        if repo_name != format!("homebrew-{}", tap) {
            eprintln!(
                "Note: repo name does not match homebrew-<short>; 'brew tap {}/{}' shorthand may not work.",
                owner, tap
            );
        }

        Ok(Self {
            owner,
            tap,
            repo_name,
            visibility,
            branch,
            formula_mode,
            formula_url,
            formula_name,
        })
    }

    pub fn repo_slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo_name)
    }
}

fn normalize_token(label: &str, value: String) -> Result<String> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        bail!("{} is required", label);
    }

    if trimmed.contains('/') {
        bail!("{} must not include '/'", label);
    }

    if trimmed.chars().any(|ch| ch.is_whitespace()) {
        bail!("{} must not contain whitespace", label);
    }

    Ok(trimmed.to_string())
}

fn normalize_branch(branch: String) -> Result<String> {
    let trimmed = branch.trim();

    if trimmed.is_empty() {
        bail!("branch is required");
    }

    Ok(trimmed.to_string())
}
