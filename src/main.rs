mod graphreader;
mod icedgraph;

use std::env;
use iced::Settings;
use iced::Application;
use std::path::PathBuf;

use icedgraph::graphapp::{GraphApp, GraphAppFlags};

fn main() {
    // Read given arguments to find directory with nodes
    // Default to current directory
    let args: Vec<String> = env::args().collect();

    let notes_dir: PathBuf = match args.len() {
        1 => {
            println!("Using default dir");
            PathBuf::from(".")
        }
        2 => {
            let d = &args[1];
            println!("Using {}", d);
            PathBuf::from(d)
        }
        _ => {
            println!("Invalid arguments");
            for a in &args {
                print!(" {}", a);
            }
            return;
        }
    };

    let graph = graphreader::generate_graph(&notes_dir);

    // Start graphical interface
    match GraphApp::run(Settings::with_flags(GraphAppFlags::from_graph(graph))) {
        Err(e) => {
            println!("{}", e);
            return;
        }
        Ok(_) => {
            return;
        }
    }
}
