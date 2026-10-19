use std::ffi::OsString;
use std::path::PathBuf;

use log::info;

use mdx::Result;
use mdx::builder::{BuilderConfig, SourceType, ZDBBuilder};

use crate::report::print_progress;

pub fn run_build_mdd(directory: &str, password: &str, file: &str) -> Result<()> {
    info!("Building MDD from directory: {directory}");

    let expanded_dir = PathBuf::from(shellexpand::tilde(directory).to_string());
    let dir_path = std::fs::canonicalize(&expanded_dir).unwrap_or(expanded_dir);

    if !dir_path.exists() {
        return Err(mdx::ZdbError::invalid_path(format!(
            "Directory does not exist: {}",
            dir_path.display()
        )));
    }

    if !dir_path.is_dir() {
        return Err(mdx::ZdbError::invalid_path(format!(
            "Path is not a directory: {}",
            dir_path.display()
        )));
    }

    // Determine output file path without stripping dots in directory names (e.g. "dict.v1.2")
    let output_file = if file.is_empty() {
        let mut os_str: OsString = dir_path.as_os_str().to_owned();
        os_str.push(".mdd");
        PathBuf::from(os_str)
    } else {
        PathBuf::from(shellexpand::tilde(file).to_string())
    };

    info!("Building MDD from directory: {}", dir_path.display());
    info!("Output file: {}", output_file.display());

    // Create builder config for MDD file
    let config = BuilderConfig {
        input_path: dir_path.to_string_lossy().to_string(),
        output_file: output_file.to_string_lossy().to_string(),
        data_source_format: SourceType::Directory,
        content_type: "Binary".to_string(), // MDD files typically contain resources
        default_sorting_locale: "root".to_string(),
        build_mdd: true,
        password: password.to_string(),
        ..Default::default()
    };

    // Use ZDBBuilder to build the MDD file with DataDirLoader
    ZDBBuilder::build_with_config(&config, Some(print_progress))?;

    info!("MDD file created successfully: {}", output_file.display());

    Ok(())
}
