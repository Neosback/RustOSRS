//! Explicit authored-content transport on the existing EditorDocument composition.
//! Not a project save, history/clock snapshot, source identity or live import operation.
mod native;
use super::{EditorSession, appearance::EditorDocument, validate_appearance_pack};
pub(crate) use native::run_cli;
use osrs_core::{provenance::Digest256, scenario::ScenarioDefinition};
use osrs_edit::{
    MapDocument, ScenarioDocument, ScenarioPlanError, SemanticTransportError, WorkspaceDocument,
};
use osrs_hd::theme::ThemePack;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const MAX_BYTES: usize = 128 * 1024 * 1024;
const FORMAT: &str = "rustosrs-authored-snapshot-v1";
const ALGORITHM: &str = "rustosrs-authored-document-sha256-v1";

#[derive(Debug)]
pub(crate) enum Error {
    Schema(serde_json::Error),
    Map(SemanticTransportError),
    Scenario(ScenarioPlanError),
    Appearance(String),
    Format(String),
    Digest(String),
    Limit,
    Mismatch {
        domain: &'static str,
        expected: String,
        actual: String,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schema(error) => write!(f, "Snapshot schema: {error}"),
            Self::Map(error) => write!(f, "Snapshot map: {error}"),
            Self::Scenario(error) => write!(f, "Snapshot scenario: {error}"),
            Self::Appearance(error) => write!(f, "Snapshot appearance: {error}"),
            Self::Format(value) => write!(f, "Unsupported snapshot format/algorithm: {value}"),
            Self::Digest(error) => write!(f, "Snapshot digest: {error}"),
            Self::Limit => f.write_str("Authored snapshot exceeds 128 MiB"),
            Self::Mismatch {
                domain,
                expected,
                actual,
            } => write!(f, "{domain} mismatch: expected {expected}, actual {actual}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Schema(e) => Some(e),
            Self::Map(e) => Some(e),
            _ => None,
        }
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Schema(e)
    }
}

/// Declared v1 content identities. Counters, history, availability and source belong to envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Identity {
    pub map: String,
    pub scenario: String,
    pub appearance: String,
    pub authored: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    format: String,
    algorithm: String,
    identity: Identity,
    map_json: String,
    /// Required JSON text: null means absent, and differs from a present empty scenario.
    scenario_json: String,
    /// Canonical ThemePack transport; asset bytes remain complete and never extracted to disk.
    appearance_zip: Vec<u8>,
}

fn hash(domain: &[u8], bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(domain);
    h.update(bytes);
    format!("{:x}", h.finalize())
}
fn digest(value: &str) -> Result<Digest256, Error> {
    Digest256::from_lower_hex(value).map_err(|e| Error::Digest(e.to_string()))
}
fn check(domain: &'static str, expected: &str, actual: &str) -> Result<(), Error> {
    digest(expected)?;
    if expected != actual {
        return Err(Error::Mismatch {
            domain,
            expected: expected.into(),
            actual: actual.into(),
        });
    }
    Ok(())
}

pub(super) fn identity(document: &EditorDocument) -> Result<Identity, Error> {
    if let Some(scenario) = document.scenario().definition() {
        scenario
            .validate()
            .map_err(|e| Error::Scenario(ScenarioPlanError::Invalid(e)))?;
    }
    validate_appearance_pack(&document.appearance).map_err(Error::Appearance)?;
    let map = document.content_digest_v1().to_string();
    let scenario = hash(
        b"rustosrs-authored-scenario-v1\0",
        &serde_json::to_vec(&document.scenario().definition())?,
    );
    let appearance = document.appearance.identity().map_err(Error::Appearance)?;
    let mut inputs = Vec::with_capacity(96);
    for value in [&map, &scenario, &appearance] {
        inputs.extend_from_slice(&digest(value)?.bytes());
    }
    let authored = hash(b"rustosrs-authored-document-v1\0", &inputs);
    Ok(Identity {
        map,
        scenario,
        appearance,
        authored,
    })
}

/// One explicit immutable capture; it owns no history, dirty markers or source/cache handles.
pub(crate) struct Frozen {
    document: EditorDocument,
}
impl Frozen {
    pub(super) fn new(document: EditorDocument) -> Self {
        Self { document }
    }
    pub(crate) fn encode(self) -> Result<String, Error> {
        let record = Record {
            format: FORMAT.into(),
            algorithm: ALGORITHM.into(),
            identity: identity(&self.document)?,
            map_json: self.document.to_snapshot_json_v1().map_err(Error::Map)?,
            scenario_json: serde_json::to_string(&self.document.scenario().definition())?,
            appearance_zip: self
                .document
                .appearance
                .to_zip()
                .map_err(Error::Appearance)?,
        };
        let text = serde_json::to_string_pretty(&record)?;
        if text.len() > MAX_BYTES {
            return Err(Error::Limit);
        }
        Ok(text)
    }
}

pub(super) fn decode(text: &str) -> Result<EditorDocument, Error> {
    if text.len() > MAX_BYTES {
        return Err(Error::Limit);
    }
    let record: Record = serde_json::from_str(text)?;
    if record.format != FORMAT {
        return Err(Error::Format(record.format));
    }
    if record.algorithm != ALGORITHM {
        return Err(Error::Format(record.algorithm));
    }
    for value in [
        &record.identity.map,
        &record.identity.scenario,
        &record.identity.appearance,
        &record.identity.authored,
    ] {
        digest(value)?;
    }
    let map = MapDocument::from_snapshot_json_v1(&record.map_json).map_err(Error::Map)?;
    let definition: Option<ScenarioDefinition> = serde_json::from_str(&record.scenario_json)?;
    let scenario = ScenarioDocument::from_definition(definition).map_err(Error::Scenario)?;
    let appearance = ThemePack::from_zip(&record.appearance_zip).map_err(Error::Appearance)?;
    validate_appearance_pack(&appearance).map_err(Error::Appearance)?;
    let document = EditorDocument {
        workspace: WorkspaceDocument::from_parts(map, scenario),
        appearance,
    };
    let actual = identity(&document)?;
    for (domain, expected, value) in [
        ("map", &record.identity.map, &actual.map),
        ("scenario", &record.identity.scenario, &actual.scenario),
        (
            "appearance",
            &record.identity.appearance,
            &actual.appearance,
        ),
        ("authored", &record.identity.authored, &actual.authored),
    ] {
        check(domain, expected, value)?;
    }
    Ok(document)
}

#[derive(Debug, Serialize)]
pub(crate) struct Verification {
    status: &'static str,
    scope: &'static str,
    identity: Identity,
    object_count: usize,
    spawn_count: usize,
    asset_count: usize,
}
pub(crate) fn verify(text: &str) -> Result<Verification, Error> {
    let session = EditorSession::from_authored_snapshot_json_v1(text)?;
    let identity = session.authored_identity_v1()?;
    let encoded = session.freeze_authored_snapshot().encode()?;
    let restored = EditorSession::from_authored_snapshot_json_v1(&encoded)?;
    let round_trip = restored.authored_identity_v1()?;
    check("round trip", &identity.authored, &round_trip.authored)?;
    Ok(Verification {
        status: "MATCH",
        scope: "map-scenario-appearance",
        identity,
        object_count: session.document().object_count(),
        spawn_count: session
            .scenario()
            .definition()
            .map_or(0, |s| s.spawns.len()),
        asset_count: session.appearance().assets.len(),
    })
}

#[cfg(test)]
pub(crate) mod tests;
