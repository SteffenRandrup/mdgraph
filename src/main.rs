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

use iced::widget::canvas::{self, Canvas, Cursor, Frame, Geometry, Stroke, Event};  // Fill, 
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
fn graph_location_extremes(graph: &ForceGraph<(), ()>) -> (f32, f32, f32, f32) {

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

    return (min_x, max_x, min_y, max_y);

}


// Enum to send messages in iced program
#[derive(Debug, Clone)]
enum GMessage {
    // GraphEvent(Event),
    GraphEvent,
}


#[derive(Debug)]
struct GraphDisplay<'a> {
    graph: &'a ForceGraph<(), ()>,
    point_radius: f32,
}

impl GraphDisplay<'_> {
    pub fn new(graph: &ForceGraph<(), ()>) -> GraphDisplay<'_> {

        GraphDisplay {
            graph,
            point_radius: 2.5,
        }
    }
}

// Canvas needs Program impl
impl<Message> canvas::Program<Message> for GraphDisplay<'_> {

    type State = ();

    // The draw function gets called all the time
    fn draw(&self, _state: &(), _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry>{
        // We prepare a new `Frame`
        let size = bounds.size();
        let mut frame = Frame::new(size);

        // Add some padding to the extreme points - looks better
        // np means no padding
        let padding = 0.1;
        let padding_factor = 1.0 + padding;
        let (min_x_np, max_x_np, min_y_np, max_y_np) = graph_location_extremes(&self.graph);
        let (min_x, max_x, min_y, max_y) = (min_x_np * padding_factor, max_x_np * padding_factor, min_y_np * padding_factor, max_y_np * padding_factor);

        // Compute the distance between extreme points and scaling factors for the window
        let x_width = max_x - min_x;
        let y_width = max_y - min_y;
        let width_factor = size.width / x_width;
        let height_factor = size.height / y_width;

        // Shift the node coordinates such that the minimum is shifted to the origin
        // Then paint the node on the canvas as a point
        for node in self.graph.node_weights() {

            let point = Point::new((node.location[0] - min_x) * width_factor, (node.location[1] - min_y) * height_factor);

            let circle = canvas::Path::circle(point, self.point_radius);
            frame.fill(&circle, Color::WHITE);
        }

        // Draw the links between nodes (edges) as lines
        for edge in self.graph.edge_references() {

            // Get the location of the nodes
            let source = self.graph[edge.source()].location;
            let target = self.graph[edge.target()].location;

            // Translate location to points on current canvas
            let source_point = Point::new((source[0] - min_x) * width_factor, (source[1] - min_y) * height_factor);
            let target_point = Point::new((target[0] - min_x) * width_factor, (target[1] - min_y) * height_factor);

            // Draw a line between the points
            let edge_path = canvas::Path::line(source_point, target_point);
            let stroke_style = Stroke::default().with_color(Color::WHITE);
            frame.stroke(&edge_path, stroke_style);

        }

        vec![frame.into_geometry()]
    }
}

struct GraphApp {
    simulation: Simulation<(), (), Undirected>,
}

// #[derive(Default)]
struct GraphAppFlags {
    notes_directory: String, // TODO shouldn't this be a Path?
}

impl Default for GraphAppFlags {
    fn default() -> Self {
        Self { notes_directory: String::from(".") }
    }
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
        }, Command::none())
    }

    fn title(&self) -> String {
        String::from("Markdown Links")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, _message: Self::Message) -> Command<Self::Message> {
        self.simulation.update(0.055);

        let mut velsum = 0.0;
        let mut count = 0.0;

        for node in self.simulation.get_graph().node_weights() {
            velsum += (node.velocity.x.powf(2.0) + node.velocity.y.powf(2.0) + node.velocity.z.powf(2.0)).sqrt();
            count += 1.0;
        }
        let mean = velsum / count;
        println!("{}", mean);

        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        return Canvas::new(GraphDisplay::new(&self.simulation.get_graph())).width(Length::Fill).height(Length::Fill).into()
    }

    // Continuously update the graph (15ms ~ 60fps)
    // Might not want to set a fixed time
    fn subscription(&self) -> Subscription<GMessage> {
        iced::time::every(std::time::Duration::from_millis(15)).map(|_| {
            GMessage::GraphEvent
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
