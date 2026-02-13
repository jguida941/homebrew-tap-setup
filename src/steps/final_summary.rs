use anyhow::Result;

use crate::inputs::FormulaMode;
use crate::runner::{Step, VerifyStatus};
use crate::state::RunContext;

pub struct FinalSummaryStep;

impl FinalSummaryStep {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FinalSummaryStep {
    fn default() -> Self {
        Self::new()
    }
}

impl Step for FinalSummaryStep {
    fn id(&self) -> &'static str {
        "final_summary"
    }

    fn description(&self) -> &'static str {
        "Final summary"
    }

    fn preflight(&self, _ctx: &mut RunContext) -> Result<()> {
        Ok(())
    }

    fn apply(&self, ctx: &mut RunContext) -> Result<()> {
        let repo_slug = ctx.inputs.repo_slug();
        let tap_name = if ctx.inputs.repo_name == format!("homebrew-{}", ctx.inputs.tap) {
            format!("{}/{}", ctx.inputs.owner, ctx.inputs.tap)
        } else {
            format!("{}/{}", ctx.inputs.owner, ctx.inputs.repo_name)
        };
        let tap_path = ctx.state.tap_path.clone().unwrap_or_else(|| "<unknown>".to_string());
        let state_path = ctx.state_store.state_path(&ctx.run_id);

        println!("\nSummary");
        println!("  Run ID: {}", ctx.run_id);
        println!("  Repo: {}", repo_slug);
        println!("  Tap path: {}", tap_path);
        println!("  State: {}", state_path.display());

        match ctx.inputs.formula_mode {
            FormulaMode::Stub => {
                println!("  Stub formula: {}/Formula/{}.rb", tap_path, ctx.inputs.tap);
            }
            FormulaMode::BrewCreate => {
                println!("  Formula directory: {}/Formula", tap_path);
            }
        }

        println!("\nNext steps");
        println!("  - Edit the formula and replace the TODO fields.");

        let install_formula = ctx
            .state
            .formula_name
            .as_deref()
            .unwrap_or(&ctx.inputs.tap);
        println!(
            "  - brew install {}/{} (once the formula URL and sha256 are valid)",
            tap_name, install_formula
        );

        ctx.state.summary_printed = true;
        ctx.persist()?;

        Ok(())
    }

    fn verify(&self, ctx: &mut RunContext) -> Result<VerifyStatus> {
        if ctx.state.summary_printed {
            Ok(VerifyStatus::Complete)
        } else {
            Ok(VerifyStatus::Incomplete)
        }
    }
}
