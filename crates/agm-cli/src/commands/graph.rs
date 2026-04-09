//! `graph` command: output the dependency graph in DOT or Mermaid format,
//! or output a topological sort of nodes.

use std::path::Path;

use agm_core::error::output::ErrorOutputFormat;
use agm_core::graph::{build_graph, topological_sort};
use agm_core::renderer::{RenderFormat, render};

use super::helpers;

pub fn run(file: &Path, format: RenderFormat, topo: bool) -> i32 {
    let file_name = file.display().to_string();
    let source = helpers::read_file(file);

    let agm_file = helpers::parse_or_exit(&source, &file_name, ErrorOutputFormat::Text);

    if topo {
        // --topo: output topological order as plain text, one ID per line
        let graph = build_graph(&agm_file);
        match topological_sort(&graph) {
            Ok(order) => {
                for id in &order {
                    println!("{}", id);
                }
                helpers::EXIT_SUCCESS
            }
            Err(cycle_err) => {
                eprintln!("error: {}", cycle_err);
                helpers::EXIT_VALIDATION_ERROR
            }
        }
    } else {
        // Render graph in the requested format (DOT or Mermaid)
        let output = render(&agm_file, format);
        println!("{}", output);
        helpers::EXIT_SUCCESS
    }
}
