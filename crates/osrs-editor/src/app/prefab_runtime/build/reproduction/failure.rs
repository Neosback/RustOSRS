//! Rejected intent is reproducible evidence, never a successful or empty edit transaction.
use super::*;
use serde::{Deserialize, Serialize};

pub(super) const FORMAT: &str = "rustosrs-building-failure-v1";
pub(super) const CONTRACT: &str = "rustosrs-editor-building-failure-v1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FailureBundle {
    pub format: String,
    pub feature: String,
    pub planner_contract: String,
    pub inputs_digest: String,
    pub inputs: Inputs,
    pub expected_failure: plan::Failure,
    pub expected_initial_digest: String,
}

#[derive(Debug, Serialize)]
pub(super) struct FailureVerification {
    pub status: &'static str,
    pub feature: &'static str,
    pub outcome: &'static str,
    pub seed: u64,
    pub failure: plan::Failure,
    pub initial_digest: String,
    pub result_digest: String,
    pub operation_count: usize,
    pub captured_build: BuildIdentity,
    pub verifier_build: BuildIdentity,
    pub same_recorded_build_context: bool,
    pub source: Source,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Report {
    Success(Verification),
    Rejected(FailureVerification),
}

pub(super) fn verify_any(text: &str) -> Result<Report, Error> {
    if text.len() > MAX_BYTES {
        return Err(Error::Input(
            "Reproduction exceeds the 128 MiB transport limit".into(),
        ));
    }
    // Dispatch only. Preserve the original text for each closed decoder, including duplicate keys.
    #[derive(Deserialize)]
    struct Header {
        format: String,
    }
    match serde_json::from_str::<Header>(text)?.format.as_str() {
        super::FORMAT => super::verify(text).map(Report::Success),
        FORMAT => verify_failure(text).map(Report::Rejected),
        _ => Err(Error::Input("Unsupported reproduction format".into())),
    }
}

pub(super) fn verify_failure(text: &str) -> Result<FailureVerification, Error> {
    if text.len() > MAX_BYTES {
        return Err(Error::Input(
            "Reproduction exceeds the 128 MiB transport limit".into(),
        ));
    }
    let bundle: FailureBundle = serde_json::from_str(text)?;
    if bundle.format != FORMAT || bundle.feature != FEATURE || bundle.planner_contract != CONTRACT {
        return Err(Error::Input(
            "Unsupported failure format, feature or planner contract".into(),
        ));
    }
    digest(&bundle.expected_initial_digest)?;
    matches(
        "inputs",
        &bundle.inputs_digest,
        input_digest(&bundle.inputs)?,
    )?;
    let i = bundle.inputs;
    let (document, input, kit) = prepare(&i)?;
    let initial_digest = document.content_digest_v1().to_string();
    matches(
        "initial map",
        &bundle.expected_initial_digest,
        initial_digest.clone(),
    )?;
    let failure = match plan::plan(input, &i.candidate, &kit, &document) {
        Err(failure) => failure,
        Ok(_) => {
            return Err(Error::Mismatch {
                stage: "planning outcome",
                expected: "rejected".into(),
                actual: "accepted".into(),
            });
        }
    };
    if failure != bundle.expected_failure {
        return Err(Error::Mismatch {
            stage: "planning failure",
            expected: serde_json::to_string(&bundle.expected_failure)?,
            actual: serde_json::to_string(&failure)?,
        });
    }
    let result_digest = document.content_digest_v1().to_string();
    matches("rejected map", &initial_digest, result_digest.clone())?;
    let verifier_build = BuildIdentity::current();
    Ok(FailureVerification {
        status: "MATCH",
        feature: FEATURE,
        outcome: "rejected",
        seed: i.candidate.seed,
        failure,
        initial_digest,
        result_digest,
        operation_count: 0,
        same_recorded_build_context: i.editor_build == verifier_build,
        captured_build: i.editor_build,
        verifier_build,
        source: i.source,
    })
}

#[cfg(test)]
mod tests;
