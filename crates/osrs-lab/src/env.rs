//! Environment authority, legal tool surface and history. No raw-write or target-copy action exists.
use crate::{Action, LabTask, Score, Snapshot};
use osrs_edit::{EditHistory, MapDocument};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationMode {
    #[default]
    Visible,
    ScoreOnly,
}
#[derive(Debug, Clone, Serialize)]
pub struct LegalActions {
    pub width: u16,
    pub height: u16,
    pub planes: u8,
    pub palette: Vec<crate::PaletteEntry>,
    pub overlays: Vec<u32>,
    pub underlays: Vec<u32>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub remaining: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct Observation {
    pub current: Snapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Snapshot>,
    pub legal: LegalActions,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepResult {
    pub score: f64,
    pub reward: f64,
    pub breakdown: Score,
    pub exact: bool,
    pub done: bool,
    pub steps: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabError {
    InvalidTask(String),
    InvalidAction(String),
    BudgetExhausted,
    EmptyHistory,
}
impl std::fmt::Display for LabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LabError {}

#[derive(Clone)]
pub struct Env {
    task: LabTask,
    document: MapDocument,
    target: MapDocument,
    history: EditHistory,
    steps: u32,
    trace: Vec<Action>,
    seed: u64,
}
impl Env {
    pub fn reset(task: LabTask, seed: u64) -> Result<Self, LabError> {
        if task.format != "rustosrs-lab-task"
            || task.version != 1
            || task.width == 0
            || task.height == 0
            || task.width > 128
            || task.height > 128
            || task.planes == 0
            || task.planes > 4
            || task.budget == 0
            || task.budget > 100_000
        {
            return Err(LabError::InvalidTask(
                "unsupported format or dimensions/budget".into(),
            ));
        }
        let document = task
            .initial
            .document(task.width, task.height, task.planes)
            .map_err(LabError::InvalidTask)?;
        let target = task
            .target
            .document(task.width, task.height, task.planes)
            .map_err(LabError::InvalidTask)?;
        if document
            .height_entries()
            .map(|(c, _)| c)
            .collect::<Vec<_>>()
            != target.height_entries().map(|(c, _)| c).collect::<Vec<_>>()
        {
            return Err(LabError::InvalidTask(
                "initial and target height coverage must match".into(),
            ));
        }
        for p in &task.palette {
            if p.kind > 22 || p.size.contains(&0) {
                return Err(LabError::InvalidTask("invalid palette entry".into()));
            }
        }
        validate_slots(&document, &task).map_err(LabError::InvalidTask)?;
        validate_slots(&target, &task).map_err(LabError::InvalidTask)?;
        Ok(Self {
            task,
            document,
            target,
            history: EditHistory::new(),
            steps: 0,
            trace: vec![],
            seed,
        })
    }
    pub fn legal_mask(&self) -> LegalActions {
        LegalActions {
            width: self.task.width,
            height: self.task.height,
            planes: self.task.planes,
            palette: self.task.palette.clone(),
            overlays: self.task.overlays.clone(),
            underlays: self.task.underlays.clone(),
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            remaining: self.task.budget.saturating_sub(self.steps),
        }
    }
    pub fn observation(&self, mode: ObservationMode) -> Observation {
        Observation {
            current: Snapshot::capture(&self.document),
            target: (mode == ObservationMode::Visible).then(|| Snapshot::capture(&self.target)),
            legal: self.legal_mask(),
        }
    }
    pub fn score(&self) -> Score {
        crate::score::measure(&self.document, &self.target, &self.task)
    }
    pub fn document(&self) -> &MapDocument {
        &self.document
    }
    pub fn trace(&self) -> &[Action] {
        &self.trace
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn step(&mut self, action: Action) -> Result<StepResult, LabError> {
        if self.steps >= self.task.budget {
            return Err(LabError::BudgetExhausted);
        }
        let before = self.score().total;
        self.steps += 1;
        self.trace.push(action.clone());
        match &action {
            Action::Undo => {
                if self
                    .history
                    .undo(&mut self.document)
                    .map_err(|e| LabError::InvalidAction(format!("{e:?}")))?
                    .is_none()
                {
                    return Err(LabError::EmptyHistory);
                }
            }
            Action::Redo => {
                if self
                    .history
                    .redo(&mut self.document)
                    .map_err(|e| LabError::InvalidAction(format!("{e:?}")))?
                    .is_none()
                {
                    return Err(LabError::EmptyHistory);
                }
            }
            _ => {
                let patch = action
                    .plan(&self.document, &self.task)
                    .map_err(LabError::InvalidAction)?;
                let mut candidate = self.document.clone();
                candidate
                    .apply_patch(&patch)
                    .map_err(|e| LabError::InvalidAction(format!("{e:?}")))?;
                validate_slots(&candidate, &self.task).map_err(LabError::InvalidAction)?;
                self.history
                    .commit(&mut self.document, patch)
                    .map_err(|e| LabError::InvalidAction(format!("{e:?}")))?;
            }
        }
        let score = self.score();
        Ok(StepResult {
            score: score.total,
            reward: score.total - before - 0.001 / f64::from(self.task.budget),
            breakdown: score.clone(),
            exact: score.exact,
            done: score.exact || self.steps >= self.task.budget,
            steps: self.steps,
        })
    }
    pub fn replay(task: LabTask, seed: u64, actions: &[Action]) -> Result<Self, LabError> {
        let mut env = Self::reset(task, seed)?;
        for a in actions {
            if let Err(error) = env.step(a.clone())
                && matches!(error, LabError::BudgetExhausted | LabError::InvalidTask(_))
            {
                return Err(error);
            }
        }
        Ok(env)
    }
}
fn validate_slots(document: &MapDocument, task: &LabTask) -> Result<(), String> {
    let mut slots = std::collections::BTreeSet::new();
    let mut games = std::collections::BTreeMap::<_, usize>::new();
    for p in document.object_entries() {
        let t = p.position.tile;
        if t.x < 0 || t.y < 0 || t.x >= i32::from(task.width) || t.y >= i32::from(task.height) {
            return Err("resolved placement outside task".into());
        }
        let category = match p.kind.get() {
            0..=3 => 0,
            4..=8 => 1,
            22 => 2,
            _ => 3,
        };
        if category < 3 && !slots.insert((p.position, category)) {
            return Err("scene slot already occupied".into());
        }
        if category == 3 {
            let entry = task
                .palette
                .iter()
                .find(|e| e.id == p.object_id.get() && e.kind == p.kind.get())
                .ok_or("placement not present in palette")?;
            let (w, h) = if matches!(p.kind.get(), 10 | 11) {
                if p.orientation.get() & 1 == 1 {
                    (entry.size[1], entry.size[0])
                } else {
                    (entry.size[0], entry.size[1])
                }
            } else {
                (1, 1)
            };
            for dx in 0..w {
                for dy in 0..h {
                    let mut position = p.position;
                    position.tile.x += i32::from(dx);
                    position.tile.y += i32::from(dy);
                    if position.tile.x >= i32::from(task.width)
                        || position.tile.y >= i32::from(task.height)
                    {
                        return Err("footprint outside task".into());
                    }
                    let count = games.entry(position).or_default();
                    *count += 1;
                    if *count > 5 {
                        return Err("game-object tile capacity exceeded".into());
                    }
                }
            }
        }
    }
    Ok(())
}
