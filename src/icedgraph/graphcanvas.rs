use iced::mouse::ScrollDelta;
use iced::widget::canvas::event;
use iced::widget::canvas::{self, Cursor, Frame, Geometry, Stroke};
use iced::{Color, Point, Rectangle, Theme};

use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::petgraph::visit::{EdgeRef, IntoEdgeReferences};
use fdg_sim::ForceGraph;

use crate::icedgraph::messages::GMessage;
use std::collections::HashMap;
use log;

// Calculate the maximum and minimum coordinates for the graph nodes
fn graph_bounds<T, U>(graph: &ForceGraph<T, U>) -> Rectangle {
    let mut min_x = 0.0;
    let mut max_x = 0.0;
    let mut min_y = 0.0;
    let mut max_y = 0.0;

    // Find the smallest and large coordinates
    // this is to be used for scaling
    // Can't do a min/max, because floats might be NaN, Inf, ...
    for node in graph.node_weights() {
        let x = node.location[0];
        let y = node.location[1];

        min_x = match min_x.partial_cmp(&x).unwrap() {
            std::cmp::Ordering::Less => min_x,
            std::cmp::Ordering::Equal => x,
            std::cmp::Ordering::Greater => x,
        };
        max_x = match max_x.partial_cmp(&x).unwrap() {
            std::cmp::Ordering::Less => x,
            std::cmp::Ordering::Equal => max_x,
            std::cmp::Ordering::Greater => max_x,
        };
        min_y = match min_y.partial_cmp(&y).unwrap() {
            std::cmp::Ordering::Less => min_y,
            std::cmp::Ordering::Equal => y,
            std::cmp::Ordering::Greater => y,
        };
        max_y = match max_y.partial_cmp(&y).unwrap() {
            std::cmp::Ordering::Less => y,
            std::cmp::Ordering::Equal => max_y,
            std::cmp::Ordering::Greater => max_y,
        };
    }

    Rectangle {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

// Convert the location in canvas coordinates to graph coordinates
fn canvas_location_to_graph_location(
    graph_bounds: &Rectangle,
    point: Point,
    padding: f32,
    canvas_bounds: &Rectangle,
    zoom_level: f32,
    transpose_x: f32,
    transpose_y: f32,
) -> Point {
    let width_factor = (canvas_bounds.width - 2.0 * padding) / graph_bounds.width * zoom_level;
    let height_factor = (canvas_bounds.height - 2.0 * padding) / graph_bounds.height * zoom_level;

    return Point::new(
        (point.x - padding - transpose_x) / width_factor - (canvas_bounds.x - graph_bounds.x),
        (point.y - padding - transpose_y) / height_factor - (canvas_bounds.y - graph_bounds.y),
    );
}

// Convert graph coordinates to canvas coordinates
fn graph_location_to_canvas_location(
    graph_bounds: &Rectangle,
    point: Point,
    padding: f32,
    canvas_bounds: &Rectangle,
    zoom_level: f32,
    transpose_x: f32,
    transpose_y: f32,
) -> Point {
    let width_factor = (canvas_bounds.width - 2.0 * padding) / graph_bounds.width * zoom_level;
    let height_factor = (canvas_bounds.height - 2.0 * padding) / graph_bounds.height * zoom_level;

    // the Rectangle location is defined from the top left corner
    return Point::new(
        (point.x + (canvas_bounds.x - graph_bounds.x)) * width_factor + padding + transpose_x,
        (point.y + (canvas_bounds.y - graph_bounds.y)) * height_factor + padding + transpose_y,
    );
}

// The state is passed to the Canvas Program from the Application
// The Canvas will only read data
// It might be better to be able to send messages or have a command
// TODO find a way to set state without setting on public object
pub struct GraphState {
    pub active_node: Option<NodeIndex>,
    pub visualization_frac: f32,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            active_node: None,
            visualization_frac: 0.0,
        }
    }
}

// The Canvas
pub struct GraphDisplay<'a, N, E> {
    graph: &'a ForceGraph<N, E>, // contains a graph, which is displayed
    graph_state: &'a GraphState, // A state object for passing data from the Application
}

impl<N, E> GraphDisplay<'_, N, E> {
    pub fn new<'a>(
        graph: &'a ForceGraph<N, E>,
        graph_state: &'a GraphState,
    ) -> GraphDisplay<'a, N, E> {
        GraphDisplay { graph, graph_state }
    }
}

// State for the graph displaying canvas
// Must be public, because GraphDisplay is and relies on the state object
pub struct CanvasState {
    cursor_position: iced::Point,
    left_button_pressed: bool,
    position_drag_last: iced::Point,
    point_radius: f32,
    padding: f32,
    zoom_level: f32,
    transpose_x: f32,
    transpose_y: f32,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            cursor_position: iced::Point::default(),
            position_drag_last: iced::Point::default(),
            left_button_pressed: false,
            point_radius: 3.0,
            padding: 20.0,
            zoom_level: 1.0,
            transpose_x: 0.0,
            transpose_y: 0.0,
        }
    }
}

// Canvas needs Program impl -- Need generic types for the ForceGraph<N,E>
impl<N, E> canvas::Program<GMessage> for GraphDisplay<'_, N, E> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: iced::widget::canvas::Event,
        bound: Rectangle,
        cursor: Cursor,
    ) -> (event::Status, Option<GMessage>) {
        let graph = self.graph;

        match event {
            // Button was pressed
            canvas::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                // Safe to unwrap, because we just clicked the canvas and therefore have focus
                let cursor_position = canvas::Cursor::position(&cursor).unwrap();
                log::info!("Got cursor position after click");

                // Internally note that button is pressed and update position
                state.left_button_pressed = true;
                state.position_drag_last = cursor_position;

                // -- Find closest point in the graph to the clicked location
                // Calculate bounds for the graph
                let bounding_rectangle = graph_bounds(&graph);

                // Convert the clicked point to graph coordinates
                let clicked_point = canvas_location_to_graph_location(
                    &bounding_rectangle,
                    cursor_position,
                    state.padding,
                    &bound,
                    state.zoom_level,
                    state.transpose_x,
                    state.transpose_y,
                );

                let mut distance_map: HashMap<NodeIndex, f32> = HashMap::new();

                // Need to convert position in canvas to position in graph
                for nodei in graph.node_indices() {
                    let node = graph.node_weight(nodei).unwrap();

                    let x_distance = node.location.x - clicked_point.x;
                    let y_distance = node.location.y - clicked_point.y;

                    let distance = (x_distance.powf(2.0) + y_distance.powf(2.0)).sqrt();

                    distance_map.insert(nodei, distance);
                }
                let mini = distance_map
                    .iter()
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .unwrap();

                // Send a message that the graph was clicked and at which node
                if mini.1 < &100.0 {
                    return (
                        event::Status::Captured,
                        Some(GMessage::GraphClick(Some(*mini.0))),
                    );
                } else {
                    return (event::Status::Captured, Some(GMessage::GraphClick(None)));
                }
            }
            // In case we need to check for releasing cursor button
            canvas::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                state.left_button_pressed = false;
                (event::Status::Captured, None)
            }

            // Cursor was moved
            canvas::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                // Update state to reflect cursor position
                state.cursor_position = position;

                // If the button has not yet been released, we are draggin
                // This should tranpose the contents of the canvas
                if state.left_button_pressed {
                    state.transpose_x -= state.position_drag_last.x - position.x;
                    state.transpose_y -= state.position_drag_last.y - position.y;
                    state.position_drag_last = position;
                }
                (event::Status::Captured, None)
            }

            // When the scrollwheel is moved, we should adjust the zoom level
            canvas::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                let old_zoom_level = state.zoom_level;
                match delta {
                    ScrollDelta::Lines { y, .. } | ScrollDelta::Pixels { y, .. } => {
                        // Maybe not hardcode the maximum scale levels?
                        if y < 0.0 && state.zoom_level > 0.1 || y > 0.0 && state.zoom_level < 3.0 {
                            state.zoom_level = state.zoom_level * (1.0 + y / 30.0);

                            let cursor_to_center = cursor.position_from(bound.center()).unwrap();
                            let factor = state.zoom_level - old_zoom_level;

                            // Correct transpose values - TODO keep cursor position fixed?
                            state.transpose_x +=
                                cursor_to_center.x * factor / (old_zoom_level * old_zoom_level);
                            state.transpose_y +=
                                cursor_to_center.y * factor / (old_zoom_level * old_zoom_level);
                        }
                    }
                }

                (event::Status::Captured, None)
            }

            // Ignore all other events
            _ => (event::Status::Ignored, None),
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _bounds: Rectangle,
        _cursor: Cursor,
    ) -> iced::mouse::Interaction {
        if state.left_button_pressed {
            return iced::mouse::Interaction::Grabbing;
        }

        return iced::mouse::Interaction::Idle;
    }

    // The draw function gets called all the time
    fn draw(
        &self,
        state: &CanvasState,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        // We prepare a new `Frame`
        let size = bounds.size();
        let mut frame = Frame::new(size);

        let graph = self.graph; // Shorter name for later
        let graph_bounding_rectangle = graph_bounds(graph);

        // Draw a circle for every node in the graph
        for nodei in graph.node_indices() {
            let node = &graph[nodei];

            let canvas_point = graph_location_to_canvas_location(
                &graph_bounding_rectangle,
                Point::new(node.location[0], node.location[1]),
                state.padding,
                &bounds,
                state.zoom_level,
                state.transpose_x,
                state.transpose_y,
            );

            let circle = canvas::Path::circle(
                canvas_point,
                state.point_radius * (0.9 + 0.1 * state.zoom_level),
            );

            let mut connect_to_active = false;
            let color: Color = match self.graph_state.active_node {
                Some(active_index) => {
                    for node_index in graph.neighbors_undirected(nodei).into_iter() {
                        if active_index.eq(&node_index) || active_index.eq(&nodei) {
                            connect_to_active = true;
                        }
                    }

                    if connect_to_active {
                        theme.palette().text
                    } else {
                        theme.palette().primary
                    }
                }
                None => theme.palette().primary,
            };

            frame.fill(&circle, color);

            // Draw text if large enough
            let text_size = 1.5 * state.point_radius * state.zoom_level;
            if text_size > 8.0 {
                let text_color = match connect_to_active {
                    true => theme.palette().text,
                    false => theme.palette().primary,
                };

                let text = canvas::Text {
                    content: node.name.clone(),
                    position: Point::new(canvas_point.x + state.point_radius + 1.0, canvas_point.y),
                    size: text_size,
                    color: text_color,
                    ..Default::default()
                };

                frame.fill_text(text);
            }
        }

        // Draw edges between nodes
        for edge in graph.edge_references() {
            let color: Color = match self.graph_state.active_node {
                Some(active_index) => {
                    if active_index.eq(&edge.source()) || active_index.eq(&edge.target()) {
                        theme.palette().text
                    } else {
                        theme.palette().primary
                    }
                }
                None => theme.palette().primary,
            };

            let source = graph[edge.source()].location;
            let target = graph[edge.target()].location;

            let source_point = graph_location_to_canvas_location(
                &graph_bounding_rectangle,
                Point::new(source[0], source[1]),
                state.padding,
                &bounds,
                state.zoom_level,
                state.transpose_x,
                state.transpose_y,
            );
            let target_point = graph_location_to_canvas_location(
                &graph_bounding_rectangle,
                Point::new(target[0], target[1]),
                state.padding,
                &bounds,
                state.zoom_level,
                state.transpose_x,
                state.transpose_y,
            );

            let edge_path = canvas::Path::line(source_point, target_point);
            let stroke_style = Stroke::default().with_color(color);
            frame.stroke(&edge_path, stroke_style);

            // Animate edges to the active node with moving circle
            match self.graph_state.active_node {
                Some(active_index) => {
                    if active_index.eq(&edge.source()) || active_index.eq(&edge.target()) {
                        let intemediary_point = Point::new(
                            (target_point.x - source_point.x) * self.graph_state.visualization_frac
                                + source_point.x,
                            (target_point.y - source_point.y) * self.graph_state.visualization_frac
                                + source_point.y,
                        );
                        let inter_circle = canvas::Path::circle(
                            intemediary_point,
                            state.point_radius * (0.6 + 0.1 * state.zoom_level),
                        );
                        frame.fill(&inter_circle, theme.palette().success);
                    }
                }
                _ => {}
            }
        }

        vec![frame.into_geometry()]
    }
}
