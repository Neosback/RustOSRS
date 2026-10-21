//! Failure identity is assigned at the real planner boundary, never parsed from UI text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(target_arch = "wasm32"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(not(target_arch = "wasm32"), serde(rename_all = "snake_case"))]
pub(in super::super) enum Stage {
    Request,
    Generation,
    Clearing,
    MapValidation,
    CandidateValidation,
    Thumbnail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    not(target_arch = "wasm32"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(not(target_arch = "wasm32"), serde(deny_unknown_fields))]
pub(in super::super) struct Failure {
    pub(in super::super) stage: Stage,
    pub(in super::super) message: String,
    /// Exact lower-owner diagnostic evidence. It is opaque, not another error parser/model.
    pub(in super::super) cause: String,
}

impl Failure {
    pub(in super::super) fn new(stage: Stage, message: String, cause: String) -> Self {
        Self {
            stage,
            message,
            cause,
        }
    }

    pub(in super::super) fn request(message: impl Into<String>) -> Self {
        Self::new(Stage::Request, message.into(), String::new())
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Failure {}
