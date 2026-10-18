//! History-free scene initialization with the same fallible projection boundary as edits.
use super::*;
impl EditRuntime {
    pub(in crate::app) fn execute_baseline<T, E>(
        &mut self,
        patch: MapPatch,
        project: impl FnOnce(&MapDocument) -> Result<T, E>,
    ) -> Result<EditExecution<T>, EditExecutionError<E>> {
        if patch.is_empty() {
            return Ok(EditExecution::NoChange);
        }
        let backup = self.session.clone();
        let impact = patch.impact();
        let touched_regions = EditFootprint::from_patch(&patch).into_touches();
        self.session
            .apply_untracked(&patch)
            .map_err(EditExecutionError::Semantic)?;
        match project(self.session.document()) {
            Ok(projection) => Ok(EditExecution::Committed {
                receipt: EditReceipt {
                    revision: self.session.revision(),
                    impact,
                    touched_regions,
                },
                projection,
            }),
            Err(error) => {
                self.session = backup;
                Err(EditExecutionError::Projection(error))
            }
        }
    }
}
