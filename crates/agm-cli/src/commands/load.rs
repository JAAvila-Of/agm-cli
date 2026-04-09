//! `load` command: load nodes at a specific expansion level (output as JSON).

use std::path::Path;

use agm_core::error::output::ErrorOutputFormat;
use agm_core::loader::{LoadMode, load, load_nodes, load_profile};
use agm_core::renderer::{RenderFormat, render};

use super::helpers;

pub fn run(file: &Path, mode_str: &str, profile: Option<&str>, nodes: Option<&str>) -> i32 {
    let file_name = file.display().to_string();
    let source = helpers::read_file(file);

    // Parse (parse errors always in text mode for load)
    let agm_file = helpers::parse_or_exit(&source, &file_name, ErrorOutputFormat::Text);

    // Determine which loading strategy to use
    let result = if let Some(profile_name) = profile {
        // --profile overrides --mode
        load_profile(&agm_file, Some(profile_name))
    } else {
        let mode: LoadMode = match mode_str.parse() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("error: {}", e);
                return helpers::EXIT_IO_ERROR;
            }
        };

        if let Some(node_ids_str) = nodes {
            let ids: Vec<&str> = node_ids_str.split(',').map(str::trim).collect();
            Ok(load_nodes(&agm_file, mode, &ids))
        } else {
            Ok(load(&agm_file, mode))
        }
    };

    match result {
        Ok(filtered) => {
            // Use the renderer to produce canonical JSON output
            let json_output = render(&filtered, RenderFormat::Json);
            println!("{}", json_output);
            helpers::EXIT_SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e);
            helpers::EXIT_IO_ERROR
        }
    }
}
