//! The command layer: widgets never mutate the model directly — they queue
//! [`Edit`]s, and the app applies them after the UI pass. This keeps all
//! mutation (and undo bookkeeping) in one place.

use oxygrace::Project;

/// One queued model mutation.
pub struct Edit {
    /// Human label — undo menu text and the coalescing key
    /// ("axis: tick major", "set: line width", …).
    pub label: &'static str,
    /// True while a continuous gesture is in flight (slider drag, typing):
    /// successive same-label live edits share one undo snapshot.
    pub coalesce: bool,
    pub apply: Box<dyn FnOnce(&mut Project)>,
}

impl Edit {
    pub fn new<T: 'static>(
        label: &'static str,
        value: T,
        live: bool,
        set: impl FnOnce(&mut Project, T) + 'static,
    ) -> Self {
        Edit {
            label,
            coalesce: live,
            apply: Box::new(move |p| set(p, value)),
        }
    }
}
