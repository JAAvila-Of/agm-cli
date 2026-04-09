//! `render` command: render an AGM file to another format.

use std::path::Path;

use agm_core::error::output::ErrorOutputFormat;
use agm_core::renderer::{RenderFormat, render};

use super::helpers;

pub fn run(file: &Path, format: RenderFormat) -> i32 {
    let file_name = file.display().to_string();
    let source = helpers::read_file(file);

    let agm_file = helpers::parse_or_exit(&source, &file_name, ErrorOutputFormat::Text);

    let output = render(&agm_file, format);
    println!("{}", output);

    helpers::EXIT_SUCCESS
}
