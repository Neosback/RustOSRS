//! Pure dirty/navigation policy; persistence failure never dismisses the prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Destination {
    Close,
    Revert,
    Switch(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Choice {
    Save,
    Discard,
    Cancel,
}
#[derive(Debug, Default)]
pub(super) struct StudioSession {
    saved: u64,
    metadata: u64,
    saved_metadata: u64,
    pub(super) pending: Option<Destination>,
}
impl StudioSession {
    pub(super) fn baseline(&mut self, revision: u64) {
        self.saved = revision;
        self.saved_metadata = self.metadata;
        self.pending = None;
    }
    pub(super) fn mark_saved(&mut self, revision: u64) {
        self.saved = revision;
        self.saved_metadata = self.metadata;
    }
    pub(super) fn metadata_changed(&mut self) {
        self.metadata = self.metadata.wrapping_add(1);
    }
    pub(super) fn dirty(&self, revision: u64) -> bool {
        self.saved != revision || self.saved_metadata != self.metadata
    }
    pub(super) fn request(&mut self, to: Destination, revision: u64) -> Option<Destination> {
        if self.dirty(revision) {
            self.pending = Some(to);
            None
        } else {
            Some(to)
        }
    }
    pub(super) fn choose(&mut self, choice: Choice, saved: Option<u64>) -> Option<Destination> {
        match choice {
            Choice::Cancel => {
                self.pending = None;
                None
            }
            Choice::Discard => self.pending.take(),
            Choice::Save => saved.and_then(|r| {
                self.saved = r;
                self.saved_metadata = self.metadata;
                self.pending.take()
            }),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_save_and_cancel_leave_the_scene_available() {
        let mut s = StudioSession::default();
        s.baseline(2);
        assert!(s.request(Destination::Close, 3).is_none());
        assert!(s.choose(Choice::Save, None).is_none());
        assert_eq!(s.pending, Some(Destination::Close));
        assert!(s.choose(Choice::Cancel, None).is_none());
        assert!(s.dirty(3));
        assert!(s.pending.is_none());
    }
    #[test]
    fn save_and_discard_finish_only_the_requested_transition() {
        let mut s = StudioSession::default();
        assert_eq!(s.request(Destination::Close, 0), Some(Destination::Close));
        s.request(Destination::Switch("other".into()), 1);
        s.mark_saved(1); // Persistence success must retain the navigation destination.
        assert_eq!(
            s.choose(Choice::Save, Some(1)),
            Some(Destination::Switch("other".into()))
        );
        assert!(!s.dirty(1));
        s.request(Destination::Revert, 2);
        assert_eq!(s.choose(Choice::Discard, None), Some(Destination::Revert));
    }
}
