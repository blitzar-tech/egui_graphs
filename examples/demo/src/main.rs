use std::time::Instant;

use crossbeam::channel::{unbounded, Receiver, Sender};
use eframe::{run_native, App, CreationContext};
use egui::{CollapsingHeader, Context, Pos2, ScrollArea, Slider, Ui, Vec2};
use egui_graphs::events::Event;
use egui_graphs::{to_graph, DefaultEdgeShape, DefaultNodeShape, Edge, Graph, GraphView, Node};
use fdg::fruchterman_reingold::{FruchtermanReingold, FruchtermanReingoldConfiguration};
use fdg::nalgebra::{Const, OPoint};
use fdg::{Force, ForceGraph};
use petgraph::stable_graph::{DefaultIx, EdgeIndex, NodeIndex, StableGraph};
use petgraph::Directed;
use rand::Rng;

const EVENTS_LIMIT: usize = 100;

pub struct DemoApp {
    g: Graph<(), (), Directed, DefaultIx>,
    sim: ForceGraph<f32, 2, Node<(), ()>, Edge<(), ()>>,
    force: FruchtermanReingold<f32, 2>,

    settings_simulation: SettingsSimulation,

    settings_graph: SettingsGraph,
    settings_interaction: SettingsInteraction,
    settings_navigation: SettingsNavigation,
    settings_style: SettingsStyle,

    last_events: Vec<String>,

    simulation_stopped: bool,

    fps: f64,
    last_update_time: Instant,
    frames_last_time_span: usize,

    event_publisher: Sender<Event>,
    event_consumer: Receiver<Event>,

    pan: Option<[f32; 2]>,
    zoom: Option<f32>,
}

impl DemoApp {
    fn new(_: &CreationContext<'_>) -> Self {
        let settings_graph = SettingsGraph::default();
        let settings_simulation = SettingsSimulation::default();

        let mut g = generate_random_graph(settings_graph.count_node, settings_graph.count_edge);

        let mut force = FruchtermanReingold {
            conf: FruchtermanReingoldConfiguration {
                dt: settings_simulation.dt,
                cooloff_factor: settings_simulation.cooloff_factor,
                scale: settings_simulation.scale,
            },
            ..Default::default()
        };
        let mut sim = fdg::init_force_graph_uniform(g.g.clone(), 1.0);
        force.apply(&mut sim);
        g.g.node_weights_mut().for_each(|node| {
            let point: fdg::nalgebra::OPoint<f32, fdg::nalgebra::Const<2>> =
                sim.node_weight(node.id()).unwrap().1;
            node.set_location(Pos2::new(point.coords.x, point.coords.y));
        });

        let (event_publisher, event_consumer) = unbounded();

        Self {
            g,
            sim,
            force,

            event_consumer,
            event_publisher,

            settings_graph,
            settings_simulation,

            settings_interaction: SettingsInteraction::default(),
            settings_navigation: SettingsNavigation::default(),
            settings_style: SettingsStyle::default(),

            last_events: Vec::default(),

            simulation_stopped: false,

            fps: 0.,
            last_update_time: Instant::now(),
            frames_last_time_span: 0,

            pan: Option::default(),
            zoom: Option::default(),
        }
    }

    /// applies forces if simulation is running
    fn update_simulation(&mut self) {
        if self.simulation_stopped {
            return;
        }

        self.force.apply(&mut self.sim);
    }

    /// sync locations computed by the simulation with egui_graphs::Graph nodes.
    fn sync_graph_with_simulation(&mut self) {
        self.g.g.node_weights_mut().for_each(|node| {
            let sim_computed_point: OPoint<f32, Const<2>> =
                self.sim.node_weight(node.id()).unwrap().1;
            node.set_location(Pos2::new(
                sim_computed_point.coords.x,
                sim_computed_point.coords.y,
            ));
        });
    }

    fn update_fps(&mut self) {
        self.frames_last_time_span += 1;
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update_time);
        if elapsed.as_secs() >= 1 {
            self.last_update_time = now;
            self.fps = self.frames_last_time_span as f64 / elapsed.as_secs_f64();
            self.frames_last_time_span = 0;
        }
    }

    fn handle_events(&mut self) {
        self.event_consumer.try_iter().for_each(|e| {
            if self.last_events.len() > EVENTS_LIMIT {
                self.last_events.remove(0);
            }
            self.last_events.push(serde_json::to_string(&e).unwrap());

            match e {
                Event::Pan(payload) => match self.pan {
                    Some(pan) => {
                        self.pan = Some([pan[0] + payload.diff[0], pan[1] + payload.diff[1]]);
                    }
                    None => {
                        self.pan = Some(payload.diff);
                    }
                },
                Event::Zoom(z) => {
                    match self.zoom {
                        Some(zoom) => {
                            self.zoom = Some(zoom + z.diff);
                        }
                        None => {
                            self.zoom = Some(z.diff);
                        }
                    };
                }
                Event::NodeMove(payload) => {
                    let node_id = NodeIndex::new(payload.id);

                    self.sim.node_weight_mut(node_id).unwrap().1.coords.x = payload.new_pos[0];
                    self.sim.node_weight_mut(node_id).unwrap().1.coords.y = payload.new_pos[1];
                }
                _ => {}
            }
        });
    }

    fn random_node_idx(&self) -> Option<NodeIndex> {
        let nodes_cnt = self.g.node_count();
        if nodes_cnt == 0 {
            return None;
        }

        let random_n_idx = rand::thread_rng().gen_range(0..nodes_cnt);
        self.g.g.node_indices().nth(random_n_idx)
    }

    fn random_edge_idx(&self) -> Option<EdgeIndex> {
        let edges_cnt = self.g.edge_count();
        if edges_cnt == 0 {
            return None;
        }

        let random_e_idx = rand::thread_rng().gen_range(0..edges_cnt);
        self.g.g.edge_indices().nth(random_e_idx)
    }

    fn remove_random_node(&mut self) {
        let idx = self.random_node_idx().unwrap();
        self.remove_node(idx);
    }

    fn add_random_node(&mut self) {
        let random_n_idx = self.random_node_idx();
        if random_n_idx.is_none() {
            return;
        }

        let random_n = self.g.node(random_n_idx.unwrap()).unwrap();

        // location of new node is in in the closest surrounding of random existing node
        let mut rng = rand::thread_rng();
        let location = Pos2::new(
            random_n.location().x + 10. + rng.gen_range(0. ..50.),
            random_n.location().y + 10. + rng.gen_range(0. ..50.),
        );

        let g_idx = self.g.add_node_with_location((), location);

        let sim_node = egui_graphs::Node::new(());
        let sim_node_loc = fdg::nalgebra::Point2::new(location.x, location.y);

        let sim_idx = self.sim.add_node((sim_node, sim_node_loc));

        assert_eq!(g_idx, sim_idx);
    }

    fn remove_node(&mut self, idx: NodeIndex) {
        self.g.remove_node(idx);

        self.sim.remove_node(idx).unwrap();

        // update edges count
        self.settings_graph.count_edge = self.g.edge_count();
    }

    fn add_random_edge(&mut self) {
        let random_start = self.random_node_idx().unwrap();
        let random_end = self.random_node_idx().unwrap();

        self.add_edge(random_start, random_end);
    }

    fn add_edge(&mut self, start: NodeIndex, end: NodeIndex) {
        self.g.add_edge(start, end, ());

        self.sim.add_edge(start, end, egui_graphs::Edge::new(()));
    }

    fn remove_random_edge(&mut self) {
        let random_e_idx = self.random_edge_idx();
        if random_e_idx.is_none() {
            return;
        }
        let endpoints = self.g.edge_endpoints(random_e_idx.unwrap()).unwrap();

        self.remove_edge(endpoints.0, endpoints.1);
    }

    fn remove_edge(&mut self, start: NodeIndex, end: NodeIndex) {
        let (g_idx, _) = self.g.edges_connecting(start, end).next().unwrap();
        self.g.remove_edge(g_idx);

        let sim_idx = self.sim.find_edge(start, end).unwrap();
        self.sim.remove_edge(sim_idx).unwrap();
    }

    fn draw_section_simulation(&mut self, ui: &mut Ui) {
        CollapsingHeader::new("Simulation")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.style_mut().spacing.item_spacing = Vec2::new(0., 0.);
                    ui.label("Force-Directed Simulation is done with ");
                    ui.hyperlink_to("fdg project", "https://github.com/grantshandy/fdg");
                });

                ui.separator();
                ui.add_space(10.);

                ui.label("Config");
                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .button(match self.simulation_stopped {
                            true => "start",
                            false => "stop",
                        })
                        .clicked()
                    {
                        self.simulation_stopped = !self.simulation_stopped;
                    };
                    if ui.button("reset").clicked() {
                        self.reset();
                    }
                });

                ui.add_space(10.);

                self.draw_simulation_config_sliders(ui);
                ui.add_space(10.);
                ui.separator();
                self.draw_counts_sliders(ui);

                ui.add_space(10.);

                ui.separator();
            });
    }

    fn draw_section_widget(&mut self, ui: &mut Ui) {
        CollapsingHeader::new("Widget")
        .default_open(true)
        .show(ui, |ui| {
            CollapsingHeader::new("Navigation").default_open(true).show(ui, |ui|{
                if ui
                    .checkbox(&mut self.settings_navigation.fit_to_screen_enabled, "fit_to_screen")
                    .changed()
                    && self.settings_navigation.fit_to_screen_enabled
                {
                    self.settings_navigation.zoom_and_pan_enabled = false
                };
                ui.label("Enable fit to screen to fit the graph to the screen on every frame.");

                ui.add_space(5.);

                ui.add_enabled_ui(!self.settings_navigation.fit_to_screen_enabled, |ui| {
                    ui.vertical(|ui| {
                        ui.checkbox(&mut self.settings_navigation.zoom_and_pan_enabled, "zoom_and_pan");
                        ui.label("Zoom with ctrl + mouse wheel, pan with middle mouse drag.");
                    }).response.on_disabled_hover_text("disable fit_to_screen to enable zoom_and_pan");
                });
            });

            CollapsingHeader::new("Style").show(ui, |ui| {
                ui.checkbox(&mut self.settings_style.labels_always, "labels_always");
                ui.label("Wheter to show labels always or when interacted only.");
            });

            CollapsingHeader::new("Interaction").show(ui, |ui| {
                if ui.checkbox(&mut self.settings_interaction.dragging_enabled, "dragging_enabled").clicked() && self.settings_interaction.dragging_enabled {
                    self.settings_interaction.node_clicking_enabled = true;
                };
                ui.label("To drag use LMB click + drag on a node.");

                ui.add_space(5.);

                ui.add_enabled_ui(!(self.settings_interaction.dragging_enabled || self.settings_interaction.node_selection_enabled || self.settings_interaction.node_selection_multi_enabled), |ui| {
                    ui.vertical(|ui| {
                        ui.checkbox(&mut self.settings_interaction.node_clicking_enabled, "node_clicking_enabled");
                        ui.label("Check click events in last events");
                    }).response.on_disabled_hover_text("node click is enabled when any of the interaction is also enabled");
                });

                ui.add_space(5.);

                ui.add_enabled_ui(!self.settings_interaction.node_selection_multi_enabled, |ui| {
                    ui.vertical(|ui| {
                        if ui.checkbox(&mut self.settings_interaction.node_selection_enabled, "node_selection_enabled").clicked() && self.settings_interaction.node_selection_enabled {
                            self.settings_interaction.node_clicking_enabled = true;
                        };
                        ui.label("Enable select to select nodes with LMB click. If node is selected clicking on it again will deselect it.");
                    }).response.on_disabled_hover_text("node_selection_multi_enabled enables select");
                });

                if ui.checkbox(&mut self.settings_interaction.node_selection_multi_enabled, "node_selection_multi_enabled").changed() && self.settings_interaction.node_selection_multi_enabled {
                    self.settings_interaction.node_clicking_enabled = true;
                    self.settings_interaction.node_selection_enabled = true;
                }
                ui.label("Enable multiselect to select multiple nodes.");

                ui.add_space(5.);

                ui.add_enabled_ui(!(self.settings_interaction.edge_selection_enabled || self.settings_interaction.edge_selection_multi_enabled), |ui| {
                    ui.vertical(|ui| {
                        ui.checkbox(&mut self.settings_interaction.edge_clicking_enabled, "edge_clicking_enabled");
                        ui.label("Check click events in last events");
                    }).response.on_disabled_hover_text("edge click is enabled when any of the interaction is also enabled");
                });

                ui.add_space(5.);

                ui.add_enabled_ui(!self.settings_interaction.edge_selection_multi_enabled, |ui| {
                    ui.vertical(|ui| {
                        if ui.checkbox(&mut self.settings_interaction.edge_selection_enabled, "edge_selection_enabled").clicked() && self.settings_interaction.edge_selection_enabled {
                            self.settings_interaction.edge_clicking_enabled = true;
                        };
                        ui.label("Enable select to select edges with LMB click. If edge is selected clicking on it again will deselect it.");
                    }).response.on_disabled_hover_text("edge_selection_multi_enabled enables select");
                });

                if ui.checkbox(&mut self.settings_interaction.edge_selection_multi_enabled, "edge_selection_multi_enabled").changed() && self.settings_interaction.edge_selection_multi_enabled {
                    self.settings_interaction.edge_clicking_enabled = true;
                    self.settings_interaction.edge_selection_enabled = true;
                }
                ui.label("Enable multiselect to select multiple edges.");
            });

            CollapsingHeader::new("Selected").default_open(true).show(ui, |ui| {
                ScrollArea::vertical().auto_shrink([false, true]).max_height(200.).show(ui, |ui| {
                    self.g.selected_nodes().iter().for_each(|node| {
                        ui.label(format!("{node:?}"));
                    });
                    self.g.selected_edges().iter().for_each(|edge| {
                        ui.label(format!("{edge:?}"));
                    });
                });
            });

            CollapsingHeader::new("Last Events").default_open(true).show(ui, |ui| {
                if ui.button("clear").clicked() {
                    self.last_events.clear();
                }
                ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                    self.last_events.iter().rev().for_each(|event| {
                        ui.label(event);
                    });
                });
            });
        });
    }

    fn draw_section_debug(&mut self, ui: &mut Ui) {
        CollapsingHeader::new("Debug")
            .default_open(true)
            .show(ui, |ui| {
                if let Some(zoom) = self.zoom {
                    ui.label(format!("zoom: {:.5}", zoom));
                };
                if let Some(pan) = self.pan {
                    ui.label(format!("pan: [{:.5}, {:.5}]", pan[0], pan[1]));
                };

                ui.label(format!("FPS: {:.1}", self.fps));
            });
    }

    fn draw_counts_sliders(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let before = self.settings_graph.count_node as i32;

            ui.add(Slider::new(&mut self.settings_graph.count_node, 1..=2500).text("nodes"));

            let delta = self.settings_graph.count_node as i32 - before;
            (0..delta.abs()).for_each(|_| {
                if delta > 0 {
                    self.add_random_node();
                    return;
                };
                self.remove_random_node();
            });
        });

        ui.horizontal(|ui| {
            let before = self.settings_graph.count_edge as i32;

            ui.add(Slider::new(&mut self.settings_graph.count_edge, 0..=5000).text("edges"));

            let delta = self.settings_graph.count_edge as i32 - before;
            (0..delta.abs()).for_each(|_| {
                if delta > 0 {
                    self.add_random_edge();
                    return;
                };
                self.remove_random_edge();
            });
        });
    }

    fn draw_simulation_config_sliders(&mut self, ui: &mut Ui) {
        let mut changed = false;

        ui.horizontal(|ui| {
            let resp = ui.add(Slider::new(&mut self.settings_simulation.dt, 0.00..=1.).text("dt"));
            if resp.changed() {
                changed = true
            }
        });
        ui.horizontal(|ui| {
            let resp = ui.add(
                Slider::new(&mut self.settings_simulation.cooloff_factor, 0.0..=1.)
                    .text("cooloff_factor"),
            );
            if resp.changed() {
                changed = true
            }
        });
        ui.horizontal(|ui| {
            let resp =
                ui.add(Slider::new(&mut self.settings_simulation.scale, 0.0..=300.).text("scale"));
            if resp.changed() {
                changed = true
            }
        });

        if changed {
            self.force = FruchtermanReingold {
                conf: FruchtermanReingoldConfiguration {
                    dt: self.settings_simulation.dt,
                    cooloff_factor: self.settings_simulation.cooloff_factor,
                    scale: self.settings_simulation.scale,
                },
                ..Default::default()
            };
        }
    }

    fn reset(&mut self) {
        let settings_graph = SettingsGraph::default();
        let settings_simulation = SettingsSimulation::default();

        let mut g = generate_random_graph(settings_graph.count_node, settings_graph.count_edge);

        let mut force = FruchtermanReingold {
            conf: FruchtermanReingoldConfiguration {
                dt: settings_simulation.dt,
                cooloff_factor: settings_simulation.cooloff_factor,
                scale: settings_simulation.scale,
            },
            ..Default::default()
        };
        let mut sim = fdg::init_force_graph_uniform(g.g.clone(), 1.0);
        force.apply(&mut sim);
        g.g.node_weights_mut().for_each(|node| {
            let point: fdg::nalgebra::OPoint<f32, fdg::nalgebra::Const<2>> =
                sim.node_weight(node.id()).unwrap().1;
            node.set_location(Pos2::new(point.coords.x, point.coords.y));
        });

        self.settings_simulation = settings_simulation;
        self.settings_graph = settings_graph;
        self.sim = sim;
        self.g = g;
        self.force = force;
    }
}

impl App for DemoApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        egui::SidePanel::right("right_panel")
            .min_width(250.)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    self.draw_section_simulation(ui);
                    ui.add_space(10.);
                    self.draw_section_debug(ui);
                    ui.add_space(10.);
                    self.draw_section_widget(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let settings_interaction = &egui_graphs::SettingsInteraction::new()
                .with_node_selection_enabled(self.settings_interaction.node_selection_enabled)
                .with_node_selection_multi_enabled(
                    self.settings_interaction.node_selection_multi_enabled,
                )
                .with_dragging_enabled(self.settings_interaction.dragging_enabled)
                .with_node_clicking_enabled(self.settings_interaction.node_clicking_enabled)
                .with_edge_clicking_enabled(self.settings_interaction.edge_clicking_enabled)
                .with_edge_selection_enabled(self.settings_interaction.edge_selection_enabled)
                .with_edge_selection_multi_enabled(
                    self.settings_interaction.edge_selection_multi_enabled,
                );
            let settings_navigation = &egui_graphs::SettingsNavigation::new()
                .with_zoom_and_pan_enabled(self.settings_navigation.zoom_and_pan_enabled)
                .with_fit_to_screen_enabled(self.settings_navigation.fit_to_screen_enabled)
                .with_zoom_speed(self.settings_navigation.zoom_speed);
            let settings_style = &egui_graphs::SettingsStyle::new()
                .with_labels_always(self.settings_style.labels_always);
            ui.add(
                &mut GraphView::<_, _, _, _, DefaultNodeShape, DefaultEdgeShape>::new(&mut self.g)
                    .with_interactions(settings_interaction)
                    .with_navigations(settings_navigation)
                    .with_styles(settings_style)
                    .with_events(&self.event_publisher),
            );
        });

        self.handle_events();
        self.sync_graph_with_simulation();
        self.update_simulation();
        self.update_fps();
    }
}

fn generate_random_graph(node_count: usize, edge_count: usize) -> Graph<(), ()> {
    let mut rng = rand::thread_rng();
    let mut graph = StableGraph::new();

    // add nodes
    for _ in 0..node_count {
        graph.add_node(());
    }

    // add random edges
    for _ in 0..edge_count {
        let source = rng.gen_range(0..node_count);
        let target = rng.gen_range(0..node_count);

        graph.add_edge(NodeIndex::new(source), NodeIndex::new(target), ());
    }

    to_graph(&graph)
}

fn main() {
    let native_options = eframe::NativeOptions::default();
    run_native(
        "egui_graphs_demo",
        native_options,
        Box::new(|cc| Box::new(DemoApp::new(cc))),
    )
    .unwrap();
}

struct SettingsSimulation {
    dt: f32,
    cooloff_factor: f32,
    scale: f32,
}

impl Default for SettingsSimulation {
    fn default() -> Self {
        Self {
            dt: 0.03,
            cooloff_factor: 0.7,
            scale: 100.,
        }
    }
}

struct SettingsGraph {
    pub count_node: usize,
    pub count_edge: usize,
}

impl Default for SettingsGraph {
    fn default() -> Self {
        Self {
            count_node: 25,
            count_edge: 50,
        }
    }
}

#[derive(Default)]
struct SettingsInteraction {
    pub dragging_enabled: bool,
    pub node_clicking_enabled: bool,
    pub node_selection_enabled: bool,
    pub node_selection_multi_enabled: bool,
    pub edge_clicking_enabled: bool,
    pub edge_selection_enabled: bool,
    pub edge_selection_multi_enabled: bool,
}

struct SettingsNavigation {
    pub fit_to_screen_enabled: bool,
    pub zoom_and_pan_enabled: bool,
    pub screen_padding: f32,
    pub zoom_speed: f32,
}

impl Default for SettingsNavigation {
    fn default() -> Self {
        Self {
            screen_padding: 0.3,
            zoom_speed: 0.1,
            fit_to_screen_enabled: true,
            zoom_and_pan_enabled: false,
        }
    }
}

#[derive(Default)]
struct SettingsStyle {
    pub labels_always: bool,
}
