mod graphreader;
mod icedgraph;

use iced::Application;
use iced::Settings;
use std::path::PathBuf;

use icedgraph::graphapp::{GraphApp, GraphAppFlags};

use std::{env, fs};

use log;
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Config, Root};

fn setup_logging() {
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build("./nvim.log").unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder()
            .appender("logfile")
            .build(LevelFilter::Info)).unwrap();

    log4rs::init_config(config).unwrap();
}

fn main() {
    setup_logging();

    // Read given arguments to find directory with nodes
    // Default to current directory
    let args: Vec<String> = env::args().collect();

    log::info!("Started");
    let notes_dir: PathBuf = match args.len() {
        1 => {
            let path = PathBuf::from(".");
            log::info!("Using default dir {:?}", fs::canonicalize(&path));
            path
        }
        2 => {
            let d = &args[1];
            log::info!("Using {:?}", d);
            PathBuf::from(d)
        }
        _ => {
            log::warn!("Invalid arguments received");
            return;
        }
    };

    let graph = graphreader::generate_graph(&notes_dir);

    log::info!("launching display for {:?}", notes_dir);

    // Start graphical interface
    match GraphApp::run(Settings::with_flags(GraphAppFlags::from_graph(graph))) {
        Err(e) => {
            log::warn!("{}", e);
            return;
        }
        Ok(_) => {
            log::info!("Application exit successful");
            return;
        }
    }
}

