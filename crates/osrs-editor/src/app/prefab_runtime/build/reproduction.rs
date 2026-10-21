//! Freeze/replan building inputs through the existing product planner and EditRuntime.
//! Owns this operation's portable envelope, not generation, history, source loading or render truth.
mod failure;
pub(super) mod form;
mod inputs;
mod model;
use failure::verify_any;
use inputs::prepare;
#[cfg(not(target_arch = "wasm32"))]
mod native;
use super::*;
use crate::{
    app::edit_runtime::{EditReplayDirection, EditRuntime},
    session::EditorSession,
};
pub(in crate::app) use model::Source;
use model::{BuildIdentity, Bundle, Inputs, Verification};
#[cfg(not(target_arch = "wasm32"))]
pub(super) use native::ExportTask;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::run_cli;
use osrs_core::provenance::Digest256;
use serde::Serialize;
use sha2::{Digest, Sha256};
const FORMAT: &str = "rustosrs-building-reproduction-v1";
const FEATURE: &str = "buildings";
const CONTRACT: &str = "rustosrs-editor-building-plan-v1";
pub(super) const MAX_BYTES: usize = 128 * 1024 * 1024;
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub(in crate::app) enum Error {
    Schema(String),
    Input(String),
    Planning(String),
    Transaction(String),
    Mismatch {
        stage: &'static str,
        expected: String,
        actual: String,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Schema(e.to_string())
    }
}
fn digest(text: &str) -> Result<Digest256, Error> {
    Digest256::from_lower_hex(text).map_err(|e| Error::Input(e.to_string()))
}
fn input_digest(inputs: &Inputs) -> Result<String, Error> {
    let mut h = Sha256::new();
    h.update(b"rustosrs-building-inputs-v1\0");
    h.update(serde_json::to_vec(inputs)?);
    Ok(format!("{:x}", h.finalize()))
}
fn matches(stage: &'static str, expected: &str, actual: String) -> Result<(), Error> {
    digest(expected)?;
    if expected != actual {
        return Err(Error::Mismatch {
            stage,
            expected: expected.into(),
            actual,
        });
    }
    Ok(())
}
fn execute(runtime: &mut EditRuntime, patch: MapPatch) -> Result<(), Error> {
    runtime
        .execute_named(FEATURE, patch, |_| Ok::<_, std::convert::Infallible>(()))
        .map_err(|e| Error::Transaction(format!("{e:?}")))?;
    Ok(())
}

/// Owned snapshot only at an explicit capture boundary. Later edits cannot rewrite this evidence.
pub(in crate::app) struct Capture {
    document: MapDocument,
    input: Arc<PreviewInput>,
    candidate: BuildForm,
    outcome: CapturedOutcome,
    kit: Kit,
    source: Source,
}
enum CapturedOutcome {
    Success { building: MapPatch, apply: MapPatch },
    Rejected(plan::Failure),
}
impl BuildState {
    pub(in crate::app) fn reproduction_seed(
        &self,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> Option<u64> {
        self.current_preview(source, revision, size_of)
            .map(|p| p.form.seed)
            .or_else(|| {
                self.current_failure(None, source, revision, size_of)
                    .map(|f| f.form.seed)
            })
    }

    pub(in crate::app) fn freeze_reproduction(
        &self,
        document: &MapDocument,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
        identity: Source,
    ) -> Result<Capture, Error> {
        let (input, candidate, outcome) =
            if let Some(preview) = self.current_preview(source, revision, size_of) {
                (
                    Arc::clone(&preview.input),
                    preview.form.clone(),
                    CapturedOutcome::Success {
                        building: preview.building.clone(),
                        apply: preview.apply.clone(),
                    },
                )
            } else if let Some(attempt) = self.current_failure(None, source, revision, size_of) {
                (
                    Arc::clone(&attempt.input),
                    attempt.form.clone(),
                    CapturedOutcome::Rejected(attempt.failure.clone()),
                )
            } else {
                return Err(Error::Input(
                    "Generate a current building attempt before capturing reproduction".into(),
                ));
            };
        self.freeze_attempt(document, input, candidate, outcome, identity)
    }

    pub(in crate::app) fn freeze_rejected_reproduction(
        &self,
        document: &MapDocument,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
        identity: Source,
        seed: u64,
    ) -> Result<Capture, Error> {
        let attempt = self
            .current_failure(Some(seed), source, revision, size_of)
            .ok_or_else(|| Error::Input("That rejected seed is no longer current".into()))?;
        self.freeze_attempt(
            document,
            Arc::clone(&attempt.input),
            attempt.form.clone(),
            CapturedOutcome::Rejected(attempt.failure.clone()),
            identity,
        )
    }

    fn freeze_attempt(
        &self,
        document: &MapDocument,
        input: Arc<PreviewInput>,
        candidate: BuildForm,
        outcome: CapturedOutcome,
        identity: Source,
    ) -> Result<Capture, Error> {
        identity.validate()?;
        let kit = self
            .kits
            .get(input.form.kit)
            .ok_or_else(|| Error::Input("Candidate kit disappeared".into()))?;
        Ok(Capture {
            document: document.clone(),
            input,
            candidate,
            outcome,
            kit: kit.clone(),
            source: identity,
        })
    }
}
impl Capture {
    pub(in crate::app) fn encode(self) -> Result<String, Error> {
        let origin = self.input.origin;
        form::validate(&self.candidate, origin).map_err(Error::Input)?;
        let initial_map_json = self
            .document
            .to_snapshot_json_v1()
            .map_err(|e| Error::Input(e.to_string()))?;
        let inputs = Inputs {
            source: self.source,
            editor_build: BuildIdentity::current(),
            source_generation: self.input.source,
            semantic_revision: self.input.revision,
            origin: [origin.x, origin.y],
            plane: self.input.plane.index().get(),
            request: self.input.form.clone(),
            candidate: self.candidate,
            kit_json: serde_json::to_string(&self.kit.file)?,
            footprints: self.input.footprints.records(),
            initial_map_json,
        };
        let inputs_digest = input_digest(&inputs)?;
        let text = match self.outcome {
            CapturedOutcome::Success { building, apply } => {
                let patch_digest = apply.content_digest_v1().to_string();
                let mut runtime = EditRuntime::new(EditorSession::new(self.document));
                execute(&mut runtime, apply)?;
                serde_json::to_string_pretty(&Bundle {
                    format: FORMAT.into(),
                    feature: FEATURE.into(),
                    planner_contract: CONTRACT.into(),
                    inputs_digest,
                    inputs,
                    expected_building_digest: building.content_digest_v1().to_string(),
                    expected_patch_digest: patch_digest,
                    expected_result_digest: runtime.document().content_digest_v1().to_string(),
                })?
            }
            CapturedOutcome::Rejected(expected_failure) => {
                serde_json::to_string_pretty(&failure::FailureBundle {
                    format: failure::FORMAT.into(),
                    feature: FEATURE.into(),
                    planner_contract: failure::CONTRACT.into(),
                    inputs_digest,
                    inputs,
                    expected_failure,
                    expected_initial_digest: self.document.content_digest_v1().to_string(),
                })?
            }
        };
        if text.len() > MAX_BYTES {
            return Err(Error::Input(
                "Reproduction exceeds the 128 MiB transport limit".into(),
            ));
        }
        Ok(text)
    }
}

pub(in crate::app) fn verify(text: &str) -> Result<Verification, Error> {
    if text.len() > MAX_BYTES {
        return Err(Error::Input(
            "Reproduction exceeds the 128 MiB transport limit".into(),
        ));
    }
    let bundle: Bundle = serde_json::from_str(text)?;
    for expected in [
        &bundle.expected_building_digest,
        &bundle.expected_patch_digest,
        &bundle.expected_result_digest,
    ] {
        digest(expected)?;
    }
    if bundle.format != FORMAT || bundle.feature != FEATURE || bundle.planner_contract != CONTRACT {
        return Err(Error::Input(
            "Unsupported reproduction format, feature or planner contract".into(),
        ));
    }
    matches(
        "inputs",
        &bundle.inputs_digest,
        input_digest(&bundle.inputs)?,
    )?;
    let i = bundle.inputs;
    let (document, input, kit) = prepare(&i)?;
    let preview = plan::plan(input, &i.candidate, &kit, &document)
        .map_err(|e| Error::Planning(e.to_string()))?;
    matches(
        "building patch",
        &bundle.expected_building_digest,
        preview.building.content_digest_v1().to_string(),
    )?;
    let patch_digest = preview.apply.content_digest_v1().to_string();
    matches(
        "apply patch",
        &bundle.expected_patch_digest,
        patch_digest.clone(),
    )?;
    let mut runtime = EditRuntime::new(EditorSession::new(document));
    execute(&mut runtime, preview.apply.clone())?;
    let result_digest = runtime.document().content_digest_v1().to_string();
    matches(
        "result map",
        &bundle.expected_result_digest,
        result_digest.clone(),
    )?;
    // Verify exact gateway undo/redo as well; projection-settle is deliberately outside this semantic contract.
    runtime
        .replay(EditReplayDirection::Undo, |_| {
            Ok::<_, std::convert::Infallible>(())
        })
        .map_err(|e| Error::Transaction(format!("Undo: {e:?}")))?;
    let initial = MapDocument::from_snapshot_json_v1(&i.initial_map_json)
        .map_err(|e| Error::Input(e.to_string()))?;
    matches(
        "undo map",
        &initial.content_digest_v1().to_string(),
        runtime.document().content_digest_v1().to_string(),
    )?;
    runtime
        .replay(EditReplayDirection::Redo, |_| {
            Ok::<_, std::convert::Infallible>(())
        })
        .map_err(|e| Error::Transaction(format!("Redo: {e:?}")))?;
    matches(
        "redo map",
        &result_digest,
        runtime.document().content_digest_v1().to_string(),
    )?;
    let verifier_build = BuildIdentity::current();
    let same_recorded_build_context = i.editor_build == verifier_build;
    Ok(Verification {
        status: "MATCH",
        feature: FEATURE,
        patch_digest,
        result_digest,
        operation_count: preview.apply.len(),
        captured_build: i.editor_build,
        verifier_build,
        same_recorded_build_context,
        source: i.source,
    })
}
#[cfg(test)]
mod tests;

impl BuildState {
    pub(in crate::app) fn reproduction_running(&self) -> bool {
        self.reproduction_export.is_some()
    }
    pub(in crate::app) fn start_reproduction(
        &mut self,
        capture: Capture,
        path: std::path::PathBuf,
    ) -> Result<(), String> {
        if self.reproduction_running() {
            return Err("A reproduction export is already running".into());
        }
        self.reproduction_export = Some(ExportTask::start(capture, path));
        Ok(())
    }
    pub(in crate::app) fn poll_reproduction(&mut self) -> Option<Result<String, String>> {
        let result = self.reproduction_export.as_ref()?.poll()?;
        self.reproduction_export = None;
        Some(result)
    }
}
