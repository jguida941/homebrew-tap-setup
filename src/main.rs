mod runner;
mod state;
mod steps;
mod inputs;

use anyhow::Result;
use clap::Parser;

use crate::inputs::{FormulaMode, Inputs, Visibility};
use crate::runner::Runner;
use crate::state::RunContext;
use crate::steps::add_formula::AddFormulaStep;
use crate::steps::brew_tap_new::BrewTapNewStep;
use crate::steps::commit_and_push::CommitAndPushStep;
use crate::steps::final_summary::FinalSummaryStep;
use crate::steps::gh_repo_create::GhRepoCreateStep;
use crate::steps::preflight::PreflightStep;
use crate::steps::validate_tap::ValidateTapStep;

#[derive(Parser, Debug)]
#[command(author, version, about = "Homebrew tap setup helper")]
struct Cli {
    #[arg(long, default_value_t = false, help = "Print actions without applying them")]
    dry_run: bool,

    #[arg(long, help = "Resume a previous run by ID")]
    resume: Option<String>,

    #[arg(long, help = "GitHub owner or org for the tap repo")]
    owner: Option<String>,

    #[arg(long, help = "Tap short name (without the homebrew- prefix)")]
    tap: Option<String>,

    #[arg(long, help = "Override repo name (defaults to homebrew-<tap>)")]
    repo_name: Option<String>,

    #[arg(long, value_enum, default_value_t = Visibility::Public)]
    visibility: Visibility,

    #[arg(long, default_value = "main")]
    branch: String,

    #[arg(long, value_enum, default_value_t = FormulaMode::Stub)]
    formula_mode: FormulaMode,

    #[arg(long, help = "Source URL for brew create (required for brew-create mode)")]
    formula_url: Option<String>,

    #[arg(long, help = "Formula name to use with brew create (optional)")]
    formula_name: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut ctx = if let Some(run_id) = cli.resume {
        RunContext::load(run_id, cli.dry_run)?
    } else {
        let owner = cli.owner.ok_or_else(|| anyhow::anyhow!("--owner is required"))?;
        let tap = cli.tap.ok_or_else(|| anyhow::anyhow!("--tap is required"))?;
        let inputs = Inputs::new(
            owner,
            tap,
            cli.repo_name,
            cli.visibility,
            cli.branch,
            cli.formula_mode,
            cli.formula_url,
            cli.formula_name,
        )?;
        RunContext::new(cli.dry_run, inputs)?
    };
    let runner = Runner::new(vec![
        Box::new(PreflightStep::new()),
        Box::new(BrewTapNewStep::new()),
        Box::new(GhRepoCreateStep::new()),
        Box::new(AddFormulaStep::new()),
        Box::new(CommitAndPushStep::new()),
        Box::new(ValidateTapStep::new()),
        Box::new(FinalSummaryStep::new()),
    ]);

    runner.run(&mut ctx)
}
