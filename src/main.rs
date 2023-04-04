use std::cmp::min;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::io::prelude::*;
use std::fs;
use std::str::FromStr;

// use fdg_sim::force::Force;
use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};

use fdg_sim::{
    petgraph::{
        visit::{EdgeRef, IntoEdgeReferences},
        Undirected,
        graph::NodeIndex,
    },
    self,
    force::handy,
};

use iced::widget::canvas::{self, Canvas, Cursor, Frame, Geometry, Stroke};  // Fill,  , Event
use iced::widget::canvas::event;
use iced::{Color, Rectangle, Theme, Length}; //, Size
use iced::executor;
use iced::{Application, Command, Element, Settings, Point, Subscription};

use gitignore;
use walkdir::WalkDir;

use grep_searcher::sinks::UTF8;
use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use grep_matcher::{Matcher,Captures};
use std::collections::HashMap;

fn get_files(notes_directory: &Path)  -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {

    // First check if the directory exists!
    if !notes_directory.exists() {
        println!("{:} does not exist", notes_directory.display());
        return Err(Box::from("Directory does not exist"));
    }

    // If the path is not a directory, we can't work with it
    if !notes_directory.is_dir() {
        println!("{:} is not a directory", notes_directory.display());
        return Err(Box::from("Path is not a directory"));
    }

    // Then check if there is a gitignore
    // If there is, we can return whatever is included as determined by the gitignore crate
    // If not we can return a list of all files
    let gitignore_path = notes_directory.join(".gitignore");
    if gitignore_path.exists() && false {
        // TODO check that we don't get an error here e.g. if .gitignore is malformed
        let gitignore_file = gitignore::File::new(&gitignore_path).unwrap();
        // TODO what happens if gitignore is empty?
        let included_files = gitignore_file.included_files().unwrap();
        return Ok(included_files);
    }
    // We don't have a gitignore, so we just list the contents
    else {
        // This is safe, because we already checked it exists and is dir
        let mut included_files = Vec::new();
        for file in WalkDir::new(&notes_directory).into_iter().filter_map(|file| file.ok()){
            let fp = file.into_path();

            if fp.is_dir() {
                continue;
            }

            // Separate out file filtering based on path or filetype
            match fp.extension() {
                None => continue,
                Some(extension) => {
                    if extension != "md" { continue; }
                }
            }
            included_files.push(fp);
        }

        if included_files.len() == 0 {
            return Err(Box::from("No md files found"));
        }
        return Ok(included_files);
    }

}

fn generate_graph(notes_directory: &String) -> ForceGraph<(),()> {

    // initialize a graph
    let mut graph: ForceGraph<(), ()> = ForceGraph::default();

    let notes_path = Path::new(notes_directory);
    let files_vec = get_files(notes_path).unwrap();

    if files_vec.len() == 0 {
        // TODO is it better to return an empty graph
        // return Err(Box::from("Cannot build graph from empty list!"));
        return graph;
    }

    // Contains the links between notes as described in files
    let mut directed_links: HashMap<String, Vec<String>> = HashMap::new();

    // We are given a list of file PathBufs
    // For each one we read the content, then look for links to other files in each line
    for file_to_search in files_vec {

        if file_to_search.is_dir() { continue; }

        // Read the file
        let mut f = fs::File::open(file_to_search.as_path()).unwrap();
        let mut file_content = Vec::new();
        f.read_to_end(&mut file_content).unwrap();

        // Match markdown wiki-links: [[id-goes-here]]
        // Match any none-whitespace character or newline within [[..]]
        let matcher = RegexMatcher::new(r"\[{2}([\S]*)\]{2}.*").unwrap();

        // Search for matches, assuming encoding is UTF-8
        let mut matches: Vec<String> = vec![];
        let s = Searcher::new().search_slice(&matcher, &file_content, UTF8(|_, line| {

            // We want capture groups, because we might extend to multiple matches per line
            // We don't want the [[,]] included, and we might change to also ignore formatting
            let mut caps = matcher.new_captures().unwrap();

            // For each capture group, we get the first group (corresponding to link id)
            // and save it
            let caps = matcher.captures_iter(line.as_bytes(), &mut caps, |local_caps| {

                // Get the 1 capture group. 0 includes entire match, and we only care about
                // what is contained in the group
                let capture_group = local_caps.get(1).unwrap();
                // let capture_group = local_caps.get(0).unwrap();

                // Get the correct bytes from the match
                // And save it amongst other matches of links
                let matched = line[capture_group].to_string();

                // If the wiki link contains a title and / or a header identifier e.g.
                // [[id#header|some title]], we remove everything after the special character
                let matched_format_point_title = matched.find("|").unwrap_or(matched.len());
                let matched_format_point_header = matched.find("#").unwrap_or(matched.len());
                let matched_format_point = min(matched_format_point_title, matched_format_point_header);
                let matched_name = &matched[0..matched_format_point];

                matches.push(String::from(matched_name));

                true // must return bool, stops iteration if return false
            });
            // TODO handle capture group result
            match caps {
                Ok(_) => {},
                Err(_) => {}
            }

            Ok(true) // Must return Result for Searcher
        }));
        // TODO Handle the Result
        match s {
            Ok(_) => {},
            Err(_) => {}
        }

        // We should always match some, since we already filtered for Markdown...
        match file_to_search.file_stem() {
            // Get the file ID fro the filename, takes som coercion
            Some(f_stem) => {

                // For now don't push something without a mtach
                directed_links.insert(String::from(f_stem.to_str().unwrap()), matches);
            },
            None => {println!("No file stem");} // This should not happen. TODO deal with it!
        }

    }

    let mut node_names: HashMap<String, NodeIndex> = HashMap::new();

    // Build relation between filename and node id
    for id in directed_links.keys() {
        let node = graph.add_force_node(id.to_string(), ());

        node_names.insert(id.to_string(), node);
    }

    // Build edges in graph
    for (key, value) in directed_links.iter() {

        // Get the first NodeIndex from its name
        let first_node = node_names[key];

        // Iterate over linked nodes
        for second_node_name in value {

            // Handle possible input errors
            if !node_names.contains_key(second_node_name) {
                println!("{} is a bad link from {}", second_node_name, key);
                continue;
            }
            // The node references itself - this causes issues for graph simulation
            if second_node_name == key {
                println!("Self reference in {}", second_node_name);
                continue;
            }

            // Fetch the linked node by name and add an edge to the graph
            let second_node = node_names[second_node_name];
            graph.add_edge(first_node, second_node, ());
        }
    }

    // Identifiy orphans (nothing linked to or from node) and notify about it
    for node_name in directed_links.keys() {
        let node = node_names[node_name];

        // get nodes both connected from the node and other nodes linking to it
        if graph.neighbors_undirected(node).count() == 0 {
            println!("{} has no neighbors", node_name);
        }
    }

    return graph;
}

// Calculate the maximum and minimum coordinates for the graph nodes
fn graph_bounds(graph: &ForceGraph<(), ()>) -> Rectangle {

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

            std::cmp::Ordering::Less => { min_x },
            std::cmp::Ordering::Equal => { x },
            std::cmp::Ordering::Greater => { x }

        };
        max_x = match max_x.partial_cmp(&x).unwrap() {

            std::cmp::Ordering::Less => { x },
            std::cmp::Ordering::Equal => { max_x },
            std::cmp::Ordering::Greater => { max_x }

        };
        min_y = match min_y.partial_cmp(&y).unwrap() {

            std::cmp::Ordering::Less => { min_y },
            std::cmp::Ordering::Equal => { y },
            std::cmp::Ordering::Greater => { y }

        };
        max_y = match max_y.partial_cmp(&y).unwrap() {

            std::cmp::Ordering::Less => { y },
            std::cmp::Ordering::Equal => { max_y },
            std::cmp::Ordering::Greater => { max_y }

        };
    }

    Rectangle { x: min_x, y: min_y, width: max_x - min_x, height: max_y - min_y }

}


fn canvas_location_to_graph_location(graph_bounds: &Rectangle, point: Point, padding: f32, canvas_bounds: &Rectangle ) -> Point {

    let width_factor = (canvas_bounds.width - 2.0 * padding) / graph_bounds.width;
    let height_factor = (canvas_bounds.height - 2.0 * padding) / graph_bounds.height;

    return Point::new(
        (point.x - padding) / width_factor - (canvas_bounds.x - graph_bounds.x),
        (point.y - padding) / height_factor - (canvas_bounds.y - graph_bounds.y)
    )
}

fn graph_location_to_canvas_location(graph_bounds: &Rectangle, point: Point, padding: f32, canvas_bounds: &Rectangle) -> Point {

    let width_factor = (canvas_bounds.width - 2.0 * padding) / graph_bounds.width;
    let height_factor = (canvas_bounds.height - 2.0 * padding) / graph_bounds.height;

    // the Rectangle location is defined from the top left corner
    return Point::new(
        (point.x + (canvas_bounds.x - graph_bounds.x)) * width_factor + padding,
        (point.y + (canvas_bounds.y - graph_bounds.y)) * height_factor + padding
    )
}


#[derive(Debug)]
struct GraphDisplay<'a> {
    graph: &'a ForceGraph<(), ()>,
    point_radius: f32,
    padding: f32,
}

impl GraphDisplay<'_> {
    pub fn new(graph: &ForceGraph<(), ()>) -> GraphDisplay<'_> {

        GraphDisplay {
            graph,
            point_radius: 2.5,
            padding: 20.0,
        }
    }
}

#[derive(Default)]
struct CanvasState {
    cursor_position: iced::Point,
    left_button_pressed: bool,
}

// Canvas needs Program impl
impl canvas::Program<GMessage> for GraphDisplay<'_> {

    type State = CanvasState;

    fn update(&self, state: &mut CanvasState, event: iced::widget::canvas::Event, bound: Rectangle, cursor: Cursor) -> (event::Status, Option<GMessage>) {

        match event {
            // Button was pressed
            canvas::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                state.left_button_pressed = true;

                let bounding_rectangle = graph_bounds(&self.graph);

                // Safe to unwrap, because we just clicked the canvas and therefore have focus
                let cursor_position = canvas::Cursor::position(&cursor).unwrap();

                let clicked_point = canvas_location_to_graph_location(&bounding_rectangle, cursor_position, self.padding, &bound);

               let mut distance_map: HashMap<NodeIndex, f32> = HashMap::new();

                // Need to convert position in canvas to position in graph
                for nodei in self.graph.node_indices() {
                    let node = self.graph.node_weight(nodei).unwrap();

                    let x_distance = node.location.x - clicked_point.x;
                    let y_distance = node.location.y - clicked_point.y;

                    let distance = (x_distance.powf(2.0) + y_distance.powf(2.0)).sqrt();

                    distance_map.insert(nodei, distance);
                }
                let mini = distance_map.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();

                if mini.1 < &100.0 {
                    return (event::Status::Captured, Some(GMessage::GraphClick(Some(*mini.0)))); // TODO is dereferenc correct?
                }
                else {
                    return (event::Status::Captured, Some(GMessage::GraphClick(None)));
                }

            },
            // In case we need to check for releasing cursor button
            canvas::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                state.left_button_pressed = false;
                (event::Status::Captured, None)
            },

            // Cursor was moved
            canvas::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {

                // Update state to reflect cursor position
                // then acknowledge
                state.cursor_position = position;
                (event::Status::Captured, None)
            },

            // Ignore all other events
            _ => {
                (event::Status::Ignored, None)
            }
        }

    }

    // The draw function gets called all the time
    fn draw(&self, _state: &CanvasState, _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry>{
        // We prepare a new `Frame`
        let size = bounds.size();
        let mut frame = Frame::new(size);

        let graph_bounding_rectangle = graph_bounds(&self.graph);

        for node in self.graph.node_weights() {
            let canvas_point = graph_location_to_canvas_location(&graph_bounding_rectangle, Point::new(node.location[0], node.location[1]), self.padding, &bounds);

            let circle = canvas::Path::circle(canvas_point, self.point_radius);
            frame.fill(&circle, Color::WHITE);
        }

        for edge in self.graph.edge_references() {
            let source = self.graph[edge.source()].location;
            let target = self.graph[edge.target()].location;

            let source_point = graph_location_to_canvas_location(&graph_bounding_rectangle, Point::new(source[0], source[1]), self.padding, &bounds);
            let target_point = graph_location_to_canvas_location(&graph_bounding_rectangle, Point::new(target[0], target[1]), self.padding, &bounds);

            let edge_path = canvas::Path::line(source_point, target_point);
            let stroke_style = Stroke::default().with_color(Color::WHITE);
            frame.stroke(&edge_path, stroke_style);
        }

        vec![frame.into_geometry()]
    }
}

struct GraphApp {
    simulation: Simulation<(), (), Undirected>,
    update_step_counter: usize,
    simulation_step_size: f32,
}

// #[derive(Default)]
struct GraphAppFlags {
    notes_directory: String, // TODO shouldn't this be a Path?
}

impl Default for GraphAppFlags {
    fn default() -> Self {
        Self {
            notes_directory: String::from("."),
        }
    }
}

// Enum to send messages in iced program
#[derive(Debug, Clone)]
enum GMessage {
    GraphClick(Option<NodeIndex>),
    GraphicsTick,
}

impl Application for GraphApp {

    type Executor = executor::Default;
    type Flags = GraphAppFlags;
    type Message = GMessage;
    type Theme = Theme;

    fn new(flags: GraphAppFlags) -> (Self, Command<Self::Message>) {


        let graph = generate_graph(&flags.notes_directory);

        let simforce = handy(200.0, 0.9, true, true);
        let params = SimulationParameters::new(200.0, fdg_sim::Dimensions::Two, simforce);

        return (Self {
            simulation: Simulation::from_graph(graph, params),
            update_step_counter: 0,
            simulation_step_size: 0.055,
            // focused_node: None,
        }, Command::none())
    }

    fn title(&self) -> String {
        String::from("Markdown Links")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {


        match message {
            // Event from Canvas
            GMessage::GraphClick( node_index_option ) => { 

                match node_index_option {
                    Some(node_index) => {
                        let node = &self.simulation.get_graph()[node_index];
                        println!("Clicked: {:?}", node);
                    },
                    None => {
                        println!("Not clicked on a node")
                    }
                }

                // We are allowed to update the graph display again
                self.update_step_counter = 0;
            },

            // Timebased tick to update simulation
            // Don't update the simulation forever
            // Save resources
            GMessage::GraphicsTick => {
                if self.update_step_counter < 1000 {
                    self.simulation.update(self.simulation_step_size);
                    self.update_step_counter += 1;
                }

            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        return Canvas::new(GraphDisplay::new(&self.simulation.get_graph())).width(Length::Fill).height(Length::Fill).into()
    }

    // Continuously update the graph (15ms ~ 60fps)
    // Might not want to set a fixed time
    fn subscription(&self) -> Subscription<Self::Message> {
        iced::time::every(std::time::Duration::from_millis(15)).map(|_| {
            GMessage::GraphicsTick
        })
    }

}


fn main() {

    // Read given arguments to find directory with nodes
    // Default to current directory
    let args: Vec<String> = env::args().collect();

    let notes_dir = match args.len() {
        1 => { 
            println!("Using default dir");
            String::from_str(".").unwrap()
        },
        2 => {
            let d = &args[1];
            println!("Using {}", d);
            String::from_str(d).unwrap()
        }
        _ => { 
            println!("Invalid arguments");
            for a in &args {
                print!(" {}", a);
            }
            return;
        }
    };

    // Start graphical interface
    match GraphApp::run(Settings::with_flags(GraphAppFlags {notes_directory: notes_dir})) {
        Err(e) => {
            println!("{}", e);
            return
        },
        Ok(_) => {
            return;
        }
    }
}
