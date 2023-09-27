use std::{collections::HashMap, f32::consts::PI};

use egui::{
    epaint::{CircleShape, CubicBezierShape, QuadraticBezierShape, TextShape},
    Color32, FontFamily, FontId, Painter, Pos2, Shape, Stroke, Vec2,
};
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex},
    EdgeType,
};

use crate::{settings::SettingsStyle, state_computed::StateComputed, Edge, Graph, Metadata, Node};

use super::layers::Layers;

/// Edge, its index and computed state
type EdgeWithMeta<'a, E> = (EdgeIndex, Edge<E>);
/// Mapping for 2 nodes and all edges between them
type EdgeMap<'a, E> = HashMap<(NodeIndex, NodeIndex), Vec<EdgeWithMeta<'a, E>>>;

pub struct Drawer<'a, N: Clone, E: Clone, Ty: EdgeType> {
    p: Painter,

    g: &'a Graph<N, E, Ty>,
    comp: &'a StateComputed,
    settings_style: &'a SettingsStyle,
    meta: &'a Metadata,
}

impl<'a, N: Clone, E: Clone, Ty: EdgeType> Drawer<'a, N, E, Ty> {
    pub fn new(
        p: Painter,
        g: &'a Graph<N, E, Ty>,
        comp: &'a StateComputed,
        settings_style: &'a SettingsStyle,
        meta: &'a Metadata,
    ) -> Self {
        Drawer {
            g,
            p,
            comp,
            settings_style,
            meta,
        }
    }

    pub fn draw(self) {
        let mut l = Layers::default();

        self.fill_layers_edges(&mut l);
        self.fill_layers_nodes(&mut l);

        l.draw(self.p)
    }

    fn fill_layers_nodes(&self, l: &mut Layers) {
        self.g.nodes_iter().for_each(|(_, n)| {
            if !n.visible() {
                return;
            }
            self.draw_node_basic(l, n);

            if !(n.selected() || n.subselected() || n.dragged() || n.folded()) {
                return;
            }
            self.draw_node_interacted(l, n);
        });
    }

    fn fill_layers_edges(&self, l: &mut Layers) {
        let mut edge_map: EdgeMap<E> = HashMap::new();

        self.g.edges_iter().for_each(|(idx, e)| {
            let (source, target) = self.g.edge_endpoints(idx).unwrap();
            // compute map with edges between 2 nodes
            edge_map
                .entry((source, target))
                .or_insert_with(Vec::new)
                .push((idx, e.clone()));
        });

        edge_map.iter().for_each(|((start, end), edges)| {
            let mut order = edges.len();
            edges.iter().for_each(|(_, e)| {
                order -= 1;

                if start == end {
                    self.draw_edge_looped(l, start, e, order);
                } else {
                    self.draw_edge_basic(l, start, end, e, order);
                }
            });
        });
    }

    fn draw_edge_looped(&self, l: &mut Layers, n_idx: &NodeIndex, e: &Edge<E>, order: usize) {
        let node = self.g.node(*n_idx).unwrap();

        if node.subfolded() {
            // we do not draw edges which are folded
            return;
        }

        let rad = self.screen_radius(node);
        let center = self.screen_location(node.location());
        let center_horizon_angle = PI / 4.;
        let y_intersect = center.y - rad * center_horizon_angle.sin();

        let edge_start = Pos2::new(center.x - rad * center_horizon_angle.cos(), y_intersect);
        let edge_end = Pos2::new(center.x + rad * center_horizon_angle.cos(), y_intersect);

        let loop_size = rad * (self.settings_style.edge_looped_size + order as f32);

        let control_point1 = Pos2::new(center.x + loop_size, center.y - loop_size);
        let control_point2 = Pos2::new(center.x - loop_size, center.y - loop_size);

        let stroke = Stroke::new(e.width() * self.meta.zoom, e.color());
        let shape = CubicBezierShape::from_points_stroke(
            [edge_end, control_point1, control_point2, edge_start],
            false,
            Color32::TRANSPARENT,
            stroke,
        );

        if !e.subselected() {
            // draw not selected
            l.add_bottom(shape);
            return;
        }

        // draw selected
        let stroke_highlighted = Stroke::new(e.width() * self.meta.zoom, e.color());
        let shape_selected = CubicBezierShape::from_points_stroke(
            [edge_end, control_point1, control_point2, edge_start],
            false,
            Color32::TRANSPARENT,
            stroke_highlighted,
        );
        l.add_top(shape_selected);
    }

    fn draw_edge_basic(
        &self,
        l: &mut Layers,
        start_idx: &NodeIndex,
        end_idx: &NodeIndex,
        e: &Edge<E>,
        order: usize,
    ) {
        let mut n_start = self.g.node(*start_idx).unwrap();
        let mut n_end = self.g.node(*end_idx).unwrap();
        let mut transparent = false;

        if (n_start.folded() || n_start.subfolded()) && n_end.subfolded() {
            return;
        }

        // if start node is in folding tree and end node not we should draw edge transparent
        // starting from the root of the folding tree
        if n_start.subfolded() && !n_end.subfolded() {
            let new_start_idx = self
                .comp
                .foldings
                .roots_by_node(*start_idx)
                .unwrap()
                .first()
                .unwrap();
            n_start = self.g.node(*new_start_idx).unwrap();
            transparent = true;
        }

        // if end node is in folding tree and start node not we should draw edge transparent
        // ending at the root of the folding tree
        if !n_start.subfolded() && n_end.subfolded() {
            let new_end_idx = self
                .comp
                .foldings
                .roots_by_node(*end_idx)
                .unwrap()
                .first()
                .unwrap();
            n_end = self.g.node(*new_end_idx).unwrap();
            transparent = true;
        }

        let loc_start = self.screen_location(n_start.location()).to_pos2();
        let loc_end = self.screen_location(n_end.location()).to_pos2();
        let rad_start = self.screen_radius(n_start);
        let rad_end = self.screen_radius(n_end);

        let vec = loc_end - loc_start;
        let dist: f32 = vec.length();
        let dir = vec / dist;

        let start_node_radius_vec = Vec2::new(rad_start, rad_start) * dir;
        let end_node_radius_vec = Vec2::new(rad_end, rad_end) * dir;

        let tip_end = loc_start + vec - end_node_radius_vec;

        let edge_start = loc_start + start_node_radius_vec;
        let edge_end = match self.g.is_directed() {
            true => tip_end - e.tip_size() * self.meta.zoom * dir,
            false => tip_end,
        };

        let mut color = e.color();
        if transparent {
            color = color.gamma_multiply(0.15);
        }

        let stroke_edge = Stroke::new(e.width() * self.meta.zoom, color);
        let stroke_tip = Stroke::new(0., color);

        // draw straight edge
        if order == 0 {
            let tip_start_1 =
                tip_end - e.tip_size() * self.meta.zoom * rotate_vector(dir, e.tip_angle());
            let tip_start_2 =
                tip_end - e.tip_size() * self.meta.zoom * rotate_vector(dir, -e.tip_angle());

            if !e.subselected() {
                //draw straight not selected
                let shape = Shape::line_segment([edge_start, edge_end], stroke_edge);
                l.add_bottom(shape);

                // draw tips for directed edges
                if self.g.is_directed() {
                    let shape_tip = Shape::convex_polygon(
                        vec![tip_end, tip_start_1, tip_start_2],
                        color,
                        stroke_tip,
                    );
                    l.add_bottom(shape_tip);
                }

                return;
            }

            // draw straight selected
            let color_highlight = e.color();
            let stroke_edge_highlighted = Stroke::new(e.width() * self.meta.zoom, color_highlight);
            let stroke_tip_highlighted = Stroke::new(0., color_highlight);
            let shape_selected =
                Shape::line_segment([edge_start, edge_end], stroke_edge_highlighted);

            l.add_top(shape_selected);

            if self.g.is_directed() {
                let shape_tip = Shape::convex_polygon(
                    vec![tip_end, tip_start_1, tip_start_2],
                    color_highlight,
                    stroke_tip_highlighted,
                );
                l.add_top(shape_tip)
            }

            return;
        }

        // draw curved edge
        let dir_perpendicular = Vec2::new(-dir.y, dir.x);
        let center_point = (edge_start + edge_end.to_vec2()).to_vec2() / 2.0;
        let control_point = (center_point
            + dir_perpendicular * e.curve_size() * self.meta.zoom * order as f32)
            .to_pos2();

        let tip_vec = control_point - tip_end;
        let tip_dir = tip_vec / tip_vec.length();
        let tip_size = e.tip_size() * self.meta.zoom;

        let arrow_tip_dir_1 = rotate_vector(tip_dir, e.tip_angle()) * tip_size;
        let arrow_tip_dir_2 = rotate_vector(tip_dir, -e.tip_angle()) * tip_size;

        let tip_start_1 = tip_end + arrow_tip_dir_1;
        let tip_start_2 = tip_end + arrow_tip_dir_2;

        let edge_end_curved = point_between(tip_start_1, tip_start_2);

        if !e.subselected() {
            // draw curved not selected
            let shape_curved = QuadraticBezierShape::from_points_stroke(
                [edge_start, control_point, edge_end_curved],
                false,
                Color32::TRANSPARENT,
                stroke_edge,
            );
            l.add_bottom(shape_curved);

            let shape_tip_curved =
                Shape::convex_polygon(vec![tip_end, tip_start_1, tip_start_2], color, stroke_tip);
            l.add_bottom(shape_tip_curved);

            return;
        }

        // draw curved selected
        let mut color_highlighted = e.color();
        if transparent {
            color_highlighted = color_highlighted.gamma_multiply(0.15);
        }
        let stroke_highlighted_edge = Stroke::new(e.width() * self.meta.zoom, color_highlighted);
        let stroke_highlighted_tip = Stroke::new(0., color_highlighted);
        let shape_curved_selected = QuadraticBezierShape::from_points_stroke(
            [edge_start, control_point, edge_end_curved],
            false,
            Color32::TRANSPARENT,
            stroke_highlighted_edge,
        );
        l.add_top(shape_curved_selected);

        let shape_curved_tip_selected = Shape::convex_polygon(
            vec![tip_end, tip_start_1, tip_start_2],
            color_highlighted,
            stroke_highlighted_tip,
        );
        l.add_top(shape_curved_tip_selected);
    }

    fn shape_label(&self, node_radius: f32, loc: Pos2, n: &Node<N>) -> Option<TextShape> {
        let color_label = self.settings_style.color_label(self.p.ctx());
        let label_pos = Pos2::new(loc.x, loc.y - node_radius * 2.);
        let label_size = node_radius;
        let galley = self.p.layout_no_wrap(
            n.label()?.clone(),
            FontId::new(label_size, FontFamily::Monospace),
            color_label,
        );

        Some(TextShape::new(label_pos, galley))
    }

    fn screen_radius(&self, n: &Node<N>) -> f32 {
        let addition = match n.folded() {
            true => n.num_folded() as f32 * self.settings_style.folded_radius_weight,
            false => n.num_connections() as f32 * self.settings_style.edge_radius_weight,
        };
        (n.radius() + addition) * self.meta.zoom
    }

    fn screen_location(&self, loc: Vec2) -> Vec2 {
        loc * self.meta.zoom + self.meta.pan
    }

    fn draw_node_basic(&self, l: &mut Layers, node: &Node<N>) {
        let color_fill = node.color();
        let color_stroke = node.color();
        let rad = self.screen_radius(node);
        let loc = self.screen_location(node.location()).to_pos2();
        let stroke = Stroke::new(1., color_stroke);
        let shape = CircleShape {
            center: loc,
            radius: rad,
            fill: color_fill,
            stroke,
        };
        l.add_bottom(shape);

        let show_label = self.settings_style.labels_always
            || node.selected()
            || node.subselected()
            || node.dragged()
            || node.folded();

        if show_label {
            if let Some(shape_label) = self.shape_label(rad, loc, node) {
                l.add_bottom(shape_label);
            }
        }
    }

    fn draw_node_interacted(&self, l: &mut Layers, node: &Node<N>) {
        let loc = self.screen_location(node.location()).to_pos2();
        let rad = self.screen_radius(node);
        let highlight_radius = rad * 1.5;
        let text_size = rad / 2.;
        let color_stroke = node.color();

        let shape_highlight_outline = CircleShape {
            center: loc,
            radius: highlight_radius,
            fill: Color32::TRANSPARENT,
            stroke: Stroke::new(rad, color_stroke),
        };

        l.add_top(shape_highlight_outline);

        if node.folded() {
            let galley = self.p.layout_no_wrap(
                node.num_folded().to_string(),
                FontId::monospace(text_size),
                self.settings_style.color_label(self.p.ctx()),
            );
            let galley_offset = rad / 4.;
            let galley_pos = Pos2::new(loc.x - galley_offset, loc.y - galley_offset);
            let shape_galley = TextShape::new(galley_pos, galley);

            l.add_top(shape_galley);
        }
    }
}

/// rotates vector by angle
fn rotate_vector(vec: Vec2, angle: f32) -> Vec2 {
    let cos = angle.cos();
    let sin = angle.sin();
    Vec2::new(cos * vec.x - sin * vec.y, sin * vec.x + cos * vec.y)
}

/// finds point exactly in the middle between 2 points
fn point_between(p1: Pos2, p2: Pos2) -> Pos2 {
    let base = p1 - p2;
    let base_len = base.length();
    let dir = base / base_len;
    p1 - (base_len / 2.) * dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_vector() {
        let vec = Vec2::new(1.0, 0.0);
        let angle = PI / 2.0;
        let rotated = rotate_vector(vec, angle);
        assert!((rotated.x - 0.0).abs() < 1e-6);
        assert!((rotated.y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_point_between() {
        let m = point_between(Pos2::new(0.0, 0.0), Pos2::new(2.0, 0.0));
        assert!((m.x - 1.0).abs() < 1e-6);
        assert!((m.y).abs() < 1e-6);
    }
}
