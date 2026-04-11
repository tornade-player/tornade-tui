use crate::views::View;

/// Navigation stack. The bottom item is always the root view (sidebar entry).
/// Detail views (AlbumDetail, ArtistDetail, etc.) are pushed on top.
pub struct NavigationStack {
    stack: Vec<View>,
}

impl NavigationStack {
    pub fn new(root: View) -> Self {
        Self { stack: vec![root] }
    }

    pub fn push(&mut self, view: View) {
        self.stack.push(view);
    }

    /// Pop the top view. Returns `None` if only the root remains.
    pub fn pop(&mut self) -> Option<View> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }

    pub fn current(&self) -> &View {
        self.stack
            .last()
            .expect("stack always has at least one item")
    }

    pub fn current_mut(&mut self) -> &mut View {
        self.stack
            .last_mut()
            .expect("stack always has at least one item")
    }

    /// Replace the entire stack with a new root view (sidebar navigation).
    pub fn replace_root(&mut self, view: View) {
        self.stack.clear();
        self.stack.push(view);
    }

    pub fn is_root(&self) -> bool {
        self.stack.len() == 1
    }

    #[allow(dead_code)]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::{View, library::LibraryState};

    fn root() -> View {
        View::Library(LibraryState::default())
    }

    #[test]
    fn new_stack_is_root() {
        let stack = NavigationStack::new(root());
        assert!(stack.is_root());
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn push_increases_depth() {
        let mut stack = NavigationStack::new(root());
        stack.push(root());
        assert!(!stack.is_root());
        assert_eq!(stack.depth(), 2);
    }

    #[test]
    fn pop_at_root_returns_none() {
        let mut stack = NavigationStack::new(root());
        assert!(stack.pop().is_none());
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn pop_above_root_returns_some() {
        let mut stack = NavigationStack::new(root());
        stack.push(root());
        assert!(stack.pop().is_some());
        assert!(stack.is_root());
    }

    #[test]
    fn replace_root_clears_stack() {
        let mut stack = NavigationStack::new(root());
        stack.push(root());
        stack.push(root());
        stack.replace_root(root());
        assert!(stack.is_root());
        assert_eq!(stack.depth(), 1);
    }
}
