//! Snapshot-based undo: the model is plain `Clone` data, so each undo step
//! stores the full pre-edit `Project`. No command inverses needed; slider
//! drags coalesce into one step via the gesture logic in `App::apply_edit`.

use oxygrace::Project;

/// Bound on stored snapshots (each is a full model clone; the dominant cost
/// is the `Dataset` vectors, fine for typical files).
const LIMIT: usize = 50;

#[derive(Default)]
pub struct UndoStack {
    undo: Vec<(Project, &'static str)>,
    redo: Vec<(Project, &'static str)>,
}

impl UndoStack {
    /// Record the pre-edit state. Clears the redo branch.
    pub fn push(&mut self, snapshot: Project, label: &'static str) {
        self.undo.push((snapshot, label));
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Label of the step `undo()` would revert (menu text / coalescing key).
    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo.last().map(|(_, l)| *l)
    }

    pub fn redo_label(&self) -> Option<&'static str> {
        self.redo.last().map(|(_, l)| *l)
    }

    /// Swap the live project with the top undo snapshot.
    pub fn undo(&mut self, live: &mut Project) -> Option<&'static str> {
        let (snapshot, label) = self.undo.pop()?;
        self.redo.push((std::mem::replace(live, snapshot), label));
        Some(label)
    }

    pub fn redo(&mut self, live: &mut Project) -> Option<&'static str> {
        let (snapshot, label) = self.redo.pop()?;
        self.undo.push((std::mem::replace(live, snapshot), label));
        Some(label)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_walks_history() {
        let mut stack = UndoStack::default();
        let mut live = Project { page_width: 100, ..Default::default() };

        stack.push(live.clone(), "step1");
        live.page_width = 200;
        stack.push(live.clone(), "step2");
        live.page_width = 300;

        assert_eq!(stack.undo_label(), Some("step2"));
        assert_eq!(stack.undo(&mut live), Some("step2"));
        assert_eq!(live.page_width, 200);
        assert_eq!(stack.undo(&mut live), Some("step1"));
        assert_eq!(live.page_width, 100);
        assert_eq!(stack.undo(&mut live), None);

        assert_eq!(stack.redo_label(), Some("step1"));
        assert_eq!(stack.redo(&mut live), Some("step1"));
        assert_eq!(live.page_width, 200);
        assert_eq!(stack.redo(&mut live), Some("step2"));
        assert_eq!(live.page_width, 300);
        assert_eq!(stack.redo(&mut live), None);

        // A new edit clears redo.
        stack.push(live.clone(), "step3");
        live.page_width = 400;
        assert_eq!(stack.redo_label(), None);
    }
}
