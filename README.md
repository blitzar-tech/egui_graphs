![build](https://github.com/blitzarx1/egui_graphs/actions/workflows/rust.yml/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/egui_graphs)](https://crates.io/crates/egui_graphs)
[![docs.rs](https://img.shields.io/docsrs/egui_graphs)](https://docs.rs/egui_graphs)

# egui_graphs
Graph visualization with rust, [petgraph](https://github.com/petgraph/petgraph) and [egui](https://github.com/emilk/egui) in its DNA.

![Screenshot 2023-04-28 at 23 14 38](https://user-images.githubusercontent.com/32969427/235233765-23b0673b-70e5-4138-9384-180804392dba.png)

The project implements a Widget for the egui framework, enabling easy visualization of interactive graphs in rust. The goal is to implement the very basic engine for graph visualization within egui, which can be easily extended and customized for your needs.

- [x] Visualization of any complex graphs;
- [x] Zooming and panning;
- [x] Node and Edge labels;
- [x] Node and edges interactions and events reporting: click, double click, select, drag;
- [x] Style configuration via egui context styles;
- [x] Dark/Light theme support via egui context styles;
- [x] Events reporting to extend the graph functionality by the user handling them;
- [x] Layots and custom layout mechanism;

## Status
The project is on track for a stable release v1.0.0. For the moment, breaking releases are very possible.

Please use master branch for the latest updates. 

Check the [demo example](https://github.com/blitzarx1/egui_graphs/tree/master/examples/demo) for the comprehensive overview of the widget possibilities.

## Layouts
In addition to the basic graph display functionality, the project provides a layout mechanism to arrange the nodes in the graph. The `Layout` trait can be implemented by the library user allowing for custom layouts. The following layouts are coming from the box:
- [x] Random layout;
- [x] Hierarchical layout;
- [ ] Force-directed layout; (coming soon)

![Screenshot 2024-10-28 at 3 57 05 PM](https://github.com/user-attachments/assets/48614f43-4436-42eb-a238-af196d2044b4)

Check the [layouts example](https://github.com/blitzarx1/egui_graphs/blob/master/examples/layouts/src/main.rs).

## Examples
### Basic setup example
The source code of the following steps can be found in the [basic example](https://github.com/blitzarx1/egui_graphs/blob/master/examples/basic/src/main.rs).
#### Step 1: Setting up the `BasicApp` struct. 
First, let's define the `BasicApp` struct that will hold the graph.
```rust 
pub struct BasicApp {
    g: egui_graphs::Graph,
}
```

#### Step 2: Implementing the `new()` function. 
Next, implement the `new()` function for the `BasicApp` struct.
```rust
impl BasicApp {
    fn new(_: &eframe::CreationContext<'_>) -> Self {
        let g = generate_graph();
        Self { g: egui_graphs::Graph::from(&g) }
    }
}
```

#### Step 3: Generating the graph. 
Create a helper function called `generate_graph()`. In this example, we create three nodes and three edges.
```rust 
fn generate_graph() -> petgraph::StableGraph<(), ()> {
    let mut g = petgraph::StableGraph::new();

    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());

    g.add_edge(a, b, ());
    g.add_edge(b, c, ());
    g.add_edge(c, a, ());

    g
}
```

#### Step 4: Implementing the `eframe::App` trait. 
Now, lets implement the `eframe::App` trait for the `BasicApp`. In the `update()` function, we create a `egui::CentralPanel` and add the `egui_graphs::GraphView` widget to it.
```rust 
impl eframe::App for BasicApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(&mut egui_graphs::GraphView::new(&mut self.g));
        });
    }
}
```

#### Step 5: Running the application. 
Finally, run the application using the `eframe::run_native()` function.
```rust 
fn main() {
    eframe::run_native(
        "egui_graphs_basic_demo",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(BasicApp::new(cc)))),
    )
    .unwrap();
}
```

![Screenshot 2023-10-14 at 23 49 49](https://github.com/blitzarx1/egui_graphs/assets/32969427/584b78de-bca3-421b-b003-9321fd3e1b13)
You can further customize the appearance and behavior of your graph by modifying the settings or adding more nodes and edges as needed.

## Features
### Events
Can be enabled with `events` feature. Events describe a change made in graph whether it changed zoom level or node dragging. 

Combining this feature with custom node draw function allows to implement custom node behavior and drawing according to the events happening.
