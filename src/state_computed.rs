use std::collections::HashMap;

use petgraph::{stable_graph::EdgeIndex, stable_graph::NodeIndex, EdgeType};

use crate::{
    graph_wrapper::GraphWrapper, metadata::Metadata, subgraphs::SubGraphs, Node,
    SettingsInteraction, SettingsStyle,
};

/// `StateComputed` is a utility struct for managing ephemerial state which is created and destroyed in one frame.
///
/// The struct stores selections, dragged node and computed elements states.
#[derive(Default, Debug, Clone)]
pub struct StateComputed {
    pub dragged: Option<NodeIndex>,
    pub selections: SubGraphs,
    pub foldings: SubGraphs,
    pub nodes: HashMap<NodeIndex, StateComputedNode>,
    pub edges: HashMap<EdgeIndex, StateComputedEdge>,
}

impl StateComputed {
    pub fn compute_for_edge(&mut self, idx: EdgeIndex) {
        self.edges.entry(idx).or_default();
    }

    pub fn compute_for_node<N: Clone, E: Clone, Ty: EdgeType>(
        &mut self,
        g: &GraphWrapper<'_, N, E, Ty>,
        meta: &mut Metadata,
        idx: NodeIndex,
        n: &Node<N>,
        settings_interaction: &SettingsInteraction,
        settings_style: &SettingsStyle,
    ) {
        self.nodes.entry(idx).or_default();

        // compute radii
        let num = g.edges_num(idx);
        let mut radius_addition = settings_style.edge_radius_weight * num as f32;

        if n.dragged() {
            self.dragged = Some(idx);
        }

        self.compute_selection(
            g,
            idx,
            n,
            settings_interaction.selection_depth > 0,
            settings_interaction.selection_depth,
        );
        self.compute_folding(g, idx, n, settings_interaction.folding_depth);

        radius_addition += self.node_state(&idx).unwrap().num_folded as f32
            * settings_style.folded_node_radius_weight;

        {
            self.nodes
                .get_mut(&idx)
                .unwrap()
                .inc_radius(radius_addition);
        }

        let comp = self.node_state(&idx).unwrap();
        let x_minus_rad = n.location().x - comp.radius(meta);
        if x_minus_rad < meta.min_x {
            meta.min_x = x_minus_rad;
        };

        let y_minus_rad = n.location().y - comp.radius(meta);
        if y_minus_rad < meta.min_y {
            meta.min_y = y_minus_rad;
        };

        let x_plus_rad = n.location().x + comp.radius(meta);
        if x_plus_rad > meta.max_x {
            meta.max_x = x_plus_rad;
        };

        let y_plus_rad = n.location().y + comp.radius(meta);
        if y_plus_rad > meta.max_y {
            meta.max_y = y_plus_rad;
        };
    }

    fn compute_selection<N: Clone, E: Clone, Ty: EdgeType>(
        &mut self,
        g: &GraphWrapper<'_, N, E, Ty>,
        root_idx: NodeIndex,
        root: &Node<N>,
        child_mode: bool,
        depth: i32,
    ) {
        if !root.selected() {
            return;
        }

        self.selections.add_subgraph(g, root_idx, depth);

        let elements = self.selections.elements_by_root(root_idx);
        if elements.is_none() {
            return;
        }

        let (nodes, edges) = elements.unwrap();

        nodes.iter().for_each(|idx| {
            if *idx == root_idx {
                return;
            }

            let computed = self.nodes.entry(*idx).or_default();
            if child_mode {
                computed.selected_child = true;
                return;
            }
            computed.selected_parent = true;
        });

        edges.iter().for_each(|idx| {
            let mut computed = self.edges.entry(*idx).or_default();
            if child_mode {
                computed.selected_child = true;
                return;
            }
            computed.selected_parent = true;
        });
    }

    fn compute_folding<N: Clone, E: Clone, Ty: EdgeType>(
        &mut self,
        g: &GraphWrapper<'_, N, E, Ty>,
        root_idx: NodeIndex,
        root: &Node<N>,
        depth: usize,
    ) {
        if !root.folded() {
            return;
        }

        let depth_normalized = match depth {
            usize::MAX => i32::MAX,
            _ => depth as i32,
        };

        self.foldings.add_subgraph(g, root_idx, depth_normalized);

        let elements = self.foldings.elements_by_root(root_idx);
        if elements.is_none() {
            return;
        }

        let (nodes, _) = elements.unwrap();
        self.nodes.entry(root_idx).or_default().num_folded = nodes.len() - 1; // dont't count root node

        nodes.iter().for_each(|idx| {
            if *idx == root_idx {
                return;
            }

            self.nodes.entry(*idx).or_default().folded_child = true;
        });
    }

    pub fn node_state(&self, idx: &NodeIndex) -> Option<&StateComputedNode> {
        self.nodes.get(idx)
    }

    pub fn edge_state(&self, idx: &EdgeIndex) -> Option<&StateComputedEdge> {
        self.edges.get(idx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateComputedNode {
    pub selected_child: bool,
    pub selected_parent: bool,
    pub folded_child: bool,
    pub num_folded: usize,
    radius: f32,
}

impl Default for StateComputedNode {
    fn default() -> Self {
        Self {
            selected_child: Default::default(),
            selected_parent: Default::default(),
            folded_child: Default::default(),
            num_folded: Default::default(),
            radius: 5.,
        }
    }
}

impl StateComputedNode {
    pub fn subselected(&self) -> bool {
        self.selected_child || self.selected_parent
    }

    pub fn subfolded(&self) -> bool {
        self.folded_child
    }

    pub fn radius(&self, meta: &Metadata) -> f32 {
        self.radius * meta.zoom
    }

    pub fn inc_radius(&mut self, inc: f32) {
        self.radius += inc;
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct StateComputedEdge {
    pub selected_child: bool,
    pub selected_parent: bool,
}

impl StateComputedEdge {
    pub fn subselected(&self) -> bool {
        self.selected_child || self.selected_parent
    }
}
