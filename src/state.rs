use std::collections::HashSet;

pub struct State {
    dragged_node: Option<usize>,
    selected_nodes: HashSet<usize>,
    selected_edges: HashSet<(usize, usize, usize)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            dragged_node: None,
            selected_nodes: HashSet::new(),
            selected_edges: HashSet::new(),
        }
    }
}

impl State {
    pub fn dragged_node(&self) -> Option<usize> {
        self.dragged_node
    }

    pub fn set_dragged_node(&mut self, idx: usize) {
        self.dragged_node = Some(idx);
    }

    pub fn selected_nodes(&self) -> &HashSet<usize> {
        &self.selected_nodes
    }

    pub fn selected_edges(&self) -> &HashSet<(usize, usize, usize)> {
        &self.selected_edges
    }

    pub fn select_node(&mut self, idx: usize) {
        self.selected_nodes.insert(idx);
    }

    pub fn select_edge(&mut self, idx: (usize, usize, usize)) {
        self.selected_edges.insert(idx);
    }
}
