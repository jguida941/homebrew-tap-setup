use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::inputs::FormulaMode;
use crate::runner::{Step, VerifyStatus};
use crate::state::RunContext;

pub struct AddFormulaStep;

impl AddFormulaStep {
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

    fn formula_dir(tap_path: &Path) -> PathBuf {
        tap_path.join("Formula")
    }

    fn stub_formula_path(ctx: &RunContext) -> Result<PathBuf> {
        let tap_path = Self::tap_path(ctx)?;
        let dir = Self::formula_dir(Path::new(tap_path));
        Ok(dir.join(format!("{}.rb", ctx.inputs.tap)))
    }

    fn write_stub(path: &Path, formula_class: &str) -> Result<()> {
        let content = format!(
            "class {formula_class} < Formula\n  desc \"TODO: add a short description\"\n  homepage \"https://example.com\"\n  url \"https://example.com/TODO.tar.gz\"\n  sha256 \"TODO\"\n  license \"MIT\"\n\n  def install\n    # TODO: install steps\n  end\n\n  test do\n    # TODO: add a test\n  end\nend\n"
        );

        fs::write(path, content)
            .with_context(|| format!("failed to write stub formula: {}", path.display()))
    }

    fn formula_class_name(tap: &str) -> String {
        tap.split(|ch: char| ch == '-' || ch == '_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn has_formula_files(dir: &Path) -> Result<bool> {
        if !dir.exists() {
            return Ok(false);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .map(|ext| ext == "rb")
                .unwrap_or(false)
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn collect_formula_names(dir: &Path) -> Result<Vec<String>> {
        let mut names = Vec::new();
        if !dir.exists() {
            return Ok(names);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|ext| ext == "rb").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }

        names.sort();
        Ok(names)
    }

    fn derive_name_from_url(url: &str) -> Option<String> {
        let url = url.split('?').next().unwrap_or(url);
        let url = url.split('#').next().unwrap_or(url);
        let filename = url.rsplit('/').next()?;

        let mut base = filename.to_string();
        for ext in [".tar.gz", ".tgz", ".tar.bz2", ".tar.xz", ".zip"] {
            if let Some(stripped) = base.strip_suffix(ext) {
                base = stripped.to_string();
                break;
            }
        }

        if let Some((prefix, suffix)) = base.rsplit_once('-') {
            let looks_like_version = suffix
                .chars()
                .next()
                .map(|ch| ch.is_ascii_digit() || ch == 'v')
                .unwrap_or(false);
            if looks_like_version {
                base = prefix.to_string();
            }
        }

        if base.is_empty() {
            None
        } else {
            Some(base)
        }
    }

    fn set_formula_name(ctx: &mut RunContext, name: String) -> Result<()> {
        ctx.state.formula_name = Some(name);
        ctx.persist()
    }
}

impl Default for AddFormulaStep {
    fn default() -> Self {
        Self::new()
    }
}

impl Step for AddFormulaStep {
    fn id(&self) -> &'static str {
        "add_formula"
    }

    fn description(&self) -> &'static str {
        "Add formula"
    }

    fn preflight(&self, ctx: &mut RunContext) -> Result<()> {
        let tap_path = Self::tap_path(ctx)?;
        let path = Path::new(tap_path);

        if !path.exists() {
            anyhow::bail!("tap path does not exist: {}", path.display());
        }

        if ctx.inputs.formula_mode == FormulaMode::BrewCreate
            && ctx.inputs.formula_url.as_deref().unwrap_or("").is_empty()
        {
            anyhow::bail!("formula-url is required for brew-create mode");
        }

        Ok(())
    }

    fn apply(&self, ctx: &mut RunContext) -> Result<()> {
        let tap_path = Self::tap_path(ctx)?;
        let tap_path = Path::new(tap_path);
        let formula_dir = Self::formula_dir(tap_path);

        match ctx.inputs.formula_mode {
            FormulaMode::Stub => {
                fs::create_dir_all(&formula_dir).with_context(|| {
                    format!("failed to create Formula directory: {}", formula_dir.display())
                })?;

                let formula_path = Self::stub_formula_path(ctx)?;
                let class_name = Self::formula_class_name(&ctx.inputs.tap);
                if !formula_path.exists() {
                    Self::write_stub(&formula_path, &class_name)?;
                }

                Self::set_formula_name(ctx, ctx.inputs.tap.clone())?;
            }
            FormulaMode::BrewCreate => {
                let url = ctx.inputs.formula_url.as_deref().unwrap_or("");
                let formula_name = ctx
                    .inputs
                    .formula_name
                    .clone()
                    .or_else(|| Self::derive_name_from_url(url))
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "formula-name is required when formula-name cannot be derived from URL"
                        )
                    })?;
                println!("    brew create --tap {} {}", ctx.inputs.repo_slug(), url);

                let status = Command::new("brew")
                    .env("HOMEBREW_EDITOR", "/usr/bin/true")
                    .env("EDITOR", "/usr/bin/true")
                    .args([
                        "create",
                        "--tap",
                        &ctx.inputs.repo_slug(),
                        "--set-name",
                        &formula_name,
                        url,
                    ])
                    .status()
                    .context("failed to run brew create")?;

                if !status.success() {
                    anyhow::bail!("brew create returned non-zero status: {:?}", status.code());
                }

                let names = Self::collect_formula_names(&formula_dir)?;
                if names.len() == 1 {
                    Self::set_formula_name(ctx, names[0].clone())?;
                } else {
                    Self::set_formula_name(ctx, formula_name)?;
                }
            }
        }

        Ok(())
    }

    fn verify(&self, ctx: &mut RunContext) -> Result<VerifyStatus> {
        let tap_path = Self::tap_path(ctx)?;
        let tap_path = Path::new(tap_path);
        let formula_dir = Self::formula_dir(tap_path);

        match ctx.inputs.formula_mode {
            FormulaMode::Stub => {
                let formula_path = Self::stub_formula_path(ctx)?;
                if formula_path.exists() {
                    Ok(VerifyStatus::Complete)
                } else {
                    Ok(VerifyStatus::Incomplete)
                }
            }
            FormulaMode::BrewCreate => {
                if Self::has_formula_files(&formula_dir)? {
                    Ok(VerifyStatus::Complete)
                } else {
                    Ok(VerifyStatus::Incomplete)
                }
            }
        }
    }
}
