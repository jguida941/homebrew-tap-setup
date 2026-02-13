use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::inputs::Inputs;

const APP_NAME: &str = "homebrew-tap-setup";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct RunContext {
    pub run_id: String,
    pub dry_run: bool,
    pub state_store: StateStore,
    pub state: State,
    pub inputs: Inputs,
}

impl RunContext {
    pub fn new(dry_run: bool, inputs: Inputs) -> Result<Self> {
        let run_id = Uuid::new_v4().to_string();
        let state_store = StateStore::new(APP_NAME)?;
        let mut state = State::new(run_id.clone());
        state.dry_run = dry_run;
        state.inputs = Some(inputs.clone());

        state_store.init_run(&run_id, &state)?;

        Ok(Self {
            run_id,
            dry_run,
            state_store,
            state,
            inputs,
        })
    }

    pub fn load(run_id: String, dry_run: bool) -> Result<Self> {
        let state_store = StateStore::new(APP_NAME)?;
        let mut state = state_store.read_state(&run_id)?;
        let inputs = state
            .inputs
            .clone()
            .ok_or_else(|| anyhow::anyhow!("state does not contain inputs"))?;

        state.dry_run = dry_run;
        state_store.write_state(&run_id, &state)?;

        Ok(Self {
            run_id,
            dry_run,
            state_store,
            state,
            inputs,
        })
    }

    pub fn persist(&self) -> Result<()> {
        self.state_store.write_state(&self.run_id, &self.state)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct State {
    pub schema_version: u32,
    pub run_id: String,
    pub started_at: String,
    pub steps: Vec<StepRecord>,
    pub dry_run: bool,
    #[serde(default)]
    pub inputs: Option<Inputs>,
    #[serde(default)]
    pub tap_path: Option<String>,
    #[serde(default)]
    pub formula_name: Option<String>,
    #[serde(default)]
    pub summary_printed: bool,
}

impl State {
    fn new(run_id: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            run_id,
            started_at: now_rfc3339(),
            steps: Vec::new(),
            dry_run: false,
            inputs: None,
            tap_path: None,
            formula_name: None,
            summary_printed: false,
        }
    }

    pub fn ensure_step(&mut self, id: &str) -> usize {
        if let Some(index) = self.steps.iter().position(|step| step.id == id) {
            index
        } else {
            self.steps.push(StepRecord::new(id));
            self.steps.len() - 1
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StepRecord {
    pub id: String,
    pub status: StepStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
    pub skipped_apply: bool,
}

impl StepRecord {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            status: StepStatus::Pending,
            started_at: None,
            finished_at: None,
            error: None,
            skipped_apply: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Complete,
    Failed,
    DryRun,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    base_dir: PathBuf,
}

impl StateStore {
    pub fn new(app_name: &str) -> Result<Self> {
        let project_dirs =
            ProjectDirs::from("", "", app_name).context("Could not resolve config directory")?;
        let base_dir = project_dirs.config_dir().to_path_buf();

        Ok(Self { base_dir })
    }

    pub fn read_state(&self, run_id: &str) -> Result<State> {
        let state_path = self.state_path_internal(run_id);
        let data = fs::read(&state_path)
            .with_context(|| format!("Failed to read state: {}", state_path.display()))?;
        let state = serde_json::from_slice(&data)
            .with_context(|| format!("Failed to parse state: {}", state_path.display()))?;
        Ok(state)
    }

    pub fn init_run(&self, run_id: &str, state: &State) -> Result<()> {
        let run_dir = self.run_dir(run_id);
        fs::create_dir_all(&run_dir)
            .with_context(|| format!("Failed to create run directory: {}", run_dir.display()))?;
        self.write_state(run_id, state)
    }

    pub fn write_state(&self, run_id: &str, state: &State) -> Result<()> {
        let state_path = self.state_path_internal(run_id);
        let data = serde_json::to_vec_pretty(state)?;
        fs::write(&state_path, data)
            .with_context(|| format!("Failed to write state: {}", state_path.display()))
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.base_dir.join("runs").join(run_id)
    }

    fn state_path_internal(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join("state.json")
    }

    pub fn state_path(&self, run_id: &str) -> PathBuf {
        self.state_path_internal(run_id)
    }

    #[allow(dead_code)]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

pub fn now_rfc3339() -> String {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    now.format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}
