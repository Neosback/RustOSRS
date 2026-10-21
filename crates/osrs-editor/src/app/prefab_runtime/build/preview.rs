//! Admission and retention of building previews; semantic planning stays in the existing planner.
use super::*;

/// The existing request key, retained with every candidate from one immutable planning input.
/// Source/revision are editor identities, not portable document or kit content hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreviewInput {
    pub(super) form: BuildForm,
    pub(super) origin: MapTile,
    pub(super) plane: SourcePlane,
    pub(super) source: u64,
    pub(super) revision: u64,
    pub(super) footprints: FootprintInputs,
}

impl PreviewInput {
    fn matches(
        &self,
        state: &BuildState,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> bool {
        self.form == state.form
            && state.site == Some(self.origin)
            && state.site_plane == self.plane.index().get()
            && self.source == source
            && self.revision == revision
            && self.footprints.matches(size_of)
            && state.kits.get(self.form.kit).is_some()
    }
}

impl BuildState {
    fn clear_preview(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending = None;
        }
        self.preview = None;
        self.candidates.clear();
        self.failures.clear();
        self.error = None;
    }

    fn required_input(
        &self,
        document: &MapDocument,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> Result<Option<Arc<PreviewInput>>, String> {
        let Some(origin) = self.site else {
            return Ok(None);
        };
        let plane = SourcePlane::new(self.site_plane).ok_or("Invalid site plane")?;
        let kit = self.kits.get(self.form.kit).ok_or("Choose a kit first")?;
        if let Some(input) = &self.made_from
            && input.matches(self, source, revision, size_of)
        {
            return Ok(Some(Arc::clone(input)));
        }
        Ok(Some(Arc::new(PreviewInput {
            form: self.form.clone(),
            origin,
            plane,
            source,
            revision,
            footprints: FootprintInputs::capture(document, &kit.file, size_of)?,
        })))
    }

    fn prepare_preview(
        &mut self,
        document: &MapDocument,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> Option<Arc<PreviewInput>> {
        match self.required_input(document, source, revision, size_of) {
            Ok(Some(input)) => Some(input),
            result => {
                self.clear_preview();
                self.made_from = None;
                if let Err(error) = result {
                    self.error = Some(error);
                }
                None
            }
        }
    }

    /// Read only a candidate for the actual current source, map, form and site. A form edit later
    /// in the same UI frame cannot expose an old candidate to apply, save or viewport drawing.
    pub(in crate::app) fn current_preview(
        &self,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> Option<&Preview> {
        self.preview
            .as_ref()
            .filter(|preview| preview.input.matches(self, source, revision, size_of))
    }

    /// Read-only verification of a prior generated patch after it was committed. This never grants
    /// apply/save admission; those operations require `current_preview` and the caller's context.
    pub(in crate::app) fn last_preview(&self) -> Option<&Preview> {
        self.preview.as_ref()
    }

    /// Same admission key as successful previews. Failed work never grants apply/save admission.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn current_failure(
        &self,
        seed: Option<u64>,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> Option<&FailedAttempt> {
        self.failures.iter().find(|attempt| {
            seed.is_none_or(|s| s == attempt.form.seed)
                && attempt.input.matches(self, source, revision, size_of)
        })
    }

    pub(in crate::app) fn failure_messages(
        &self,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> Vec<(u64, String)> {
        self.failures
            .iter()
            .filter(|f| f.input.matches(self, source, revision, size_of))
            .map(|f| (f.form.seed, format!("Seed {}: {}", f.form.seed, f.failure)))
            .collect()
    }

    pub(in crate::app) fn candidates(
        &self,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> &[Preview] {
        if self
            .made_from
            .as_ref()
            .is_some_and(|input| input.matches(self, source, revision, size_of))
        {
            &self.candidates
        } else {
            &[]
        }
    }

    pub(in crate::app) fn select_candidate(
        &mut self,
        index: usize,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) {
        if let Some(candidate) = self.candidates(source, revision, size_of).get(index)
            && candidate.input.matches(self, source, revision, size_of)
        {
            self.preview = Some(candidate.clone());
        }
    }

    /// Explicit synchronous batch/script entry. It retires every old preview before validating.
    pub(in crate::app) fn refresh_for_script(
        &mut self,
        document: &MapDocument,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) {
        self.clear_preview();
        let Some(input) = self.prepare_preview(document, source, revision, size_of) else {
            return;
        };
        self.made_from = Some(Arc::clone(&input));
        let kit = &self.kits[input.form.kit];
        match plan::plan(Arc::clone(&input), &input.form, kit, document) {
            Ok(preview) => {
                self.candidates = vec![preview.clone()];
                self.preview = Some(preview);
            }
            Err(failure) => {
                self.error = Some(failure.to_string());
                self.failures = vec![FailedAttempt {
                    form: input.form.clone(),
                    input,
                    failure,
                }];
            }
        }
    }

    /// Validate desired inputs before observing a completion, including invalid kit/plane states.
    pub(in crate::app) fn refresh_source(
        &mut self,
        document: &MapDocument,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) {
        let Some(input) = self.prepare_preview(document, source, revision, size_of) else {
            return;
        };
        if self.made_from.as_ref() != Some(&input) {
            self.clear_preview();
            self.made_from = Some(Arc::clone(&input));
            #[cfg(not(target_arch = "wasm32"))]
            {
                if self
                    .candidate_worker
                    .as_ref()
                    .is_none_or(|worker| worker.is_finished())
                {
                    self.candidate_worker = Some(candidates::Worker::new());
                }
                self.pending = self.candidate_worker.as_mut().map(|worker| {
                    worker.start(document.clone(), input, self.kits[self.form.kit].clone())
                });
            }
            #[cfg(target_arch = "wasm32")]
            {
                self.refresh_for_script(document, source, revision, size_of);
            }
            #[cfg(not(target_arch = "wasm32"))]
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(job) = &mut self.pending {
            match job.poll() {
                Ok(None) => {}
                Ok(Some(batch)) => {
                    // An obsolete completion cannot retire the current request.
                    if batch.input != input {
                        return;
                    }
                    self.pending = None;
                    if batch.previews.iter().any(|p| p.input != input)
                        || batch.failures.iter().any(|p| p.input != input)
                    {
                        self.error = Some(
                            "The preview no longer matches the map; reroll to try again".into(),
                        );
                        return;
                    }
                    self.candidates = batch.previews;
                    self.failures = batch.failures;
                    self.preview = self.candidates.first().cloned();
                    self.error = self
                        .preview
                        .is_none()
                        .then(|| {
                            self.failures
                                .first()
                                .map(|f| format!("Seed {}: {}", f.form.seed, f.failure))
                        })
                        .flatten();
                }
                Err(_) => {
                    self.pending = None;
                    self.error = Some("Could not finish the preview; reroll to try again".into());
                }
            }
        }
    }
}
