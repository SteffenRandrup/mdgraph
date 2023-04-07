use gitignore;
use walkdir::WalkDir;

use std::cmp::min;
use std::collections::HashMap;
use std::fs;
use std::io::prelude::*;

use fdg_sim::{petgraph::graph::NodeIndex, ForceGraph, ForceGraphHelper};
use grep_matcher::{Captures, Matcher};
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use std::path::{Path, PathBuf};

pub fn get_files(notes_directory: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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
        for file in WalkDir::new(&notes_directory)
            .into_iter()
            .filter_map(|file| file.ok())
        {
            let fp = file.into_path();

            if fp.is_dir() {
                continue;
            }

            // Separate out file filtering based on path or filetype
            match fp.extension() {
                None => continue,
                Some(extension) => {
                    if extension != "md" {
                        continue;
                    }
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

pub fn generate_graph(notes_directory: &PathBuf) -> ForceGraph<(), ()> {
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
        if file_to_search.is_dir() {
            continue;
        }

        // Read the file
        let mut f = fs::File::open(file_to_search.as_path()).unwrap();
        let mut file_content = Vec::new();
        f.read_to_end(&mut file_content).unwrap();

        // Match markdown wiki-links: [[id-goes-here]]
        // Match any none-whitespace character or newline within [[..]]
        let matcher = RegexMatcher::new(r"\[{2}([\S]*)\]{2}.*").unwrap();

        // Search for matches, assuming encoding is UTF-8
        let mut matches: Vec<String> = vec![];
        let s = Searcher::new().search_slice(
            &matcher,
            &file_content,
            UTF8(|_, line| {
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
                    let matched_format_point =
                        min(matched_format_point_title, matched_format_point_header);
                    let matched_name = &matched[0..matched_format_point];

                    matches.push(String::from(matched_name));

                    true // must return bool, stops iteration if return false
                });
                // TODO handle capture group result
                match caps {
                    Ok(_) => {}
                    Err(_) => {}
                }

                Ok(true) // Must return Result for Searcher
            }),
        );
        // TODO Handle the Result
        match s {
            Ok(_) => {}
            Err(_) => {}
        }

        // We should always match some, since we already filtered for Markdown...
        match file_to_search.file_stem() {
            // Get the file ID fro the filename, takes som coercion
            Some(f_stem) => {
                // For now don't push something without a mtach
                directed_links.insert(String::from(f_stem.to_str().unwrap()), matches);
            }
            None => {
                println!("No file stem");
            } // This should not happen. TODO deal with it!
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
