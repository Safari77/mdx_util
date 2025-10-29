use std::path::PathBuf;

use log::info;

use mdx::builder::{BuilderConfig, SourceType, ZDBBuilder};
use mdx::Result;

use crate::report::print_progress;

pub fn run_build_mdd(directory: &str, password: &str, file: &str) -> Result<()> {
    info!("Building MDD from directory: {}", directory);
    
    let dir_path = PathBuf::from(shellexpand::tilde(directory).to_string());
    
    if !dir_path.exists() {
        return Err(mdx::ZdbError::invalid_path(format!("Directory does not exist: {}", dir_path.display())));
    }
    
    if !dir_path.is_dir() {
        return Err(mdx::ZdbError::invalid_path(format!("Path is not a directory: {}", dir_path.display())));
    }
    
    // Determine output file path
    let output_file = if file.is_empty() {
        dir_path.with_extension("mdd")
    } else {
        PathBuf::from(shellexpand::tilde(file).to_string())
    };
    
    info!("Building MDD from directory: {}", dir_path.display());
    info!("Output file: {}", output_file.display());
    
    // Create builder config for MDD file
    let mut config = BuilderConfig::default();
    config.input_path = dir_path.to_string_lossy().to_string();
    config.output_file = output_file.to_string_lossy().to_string();
    config.data_source_format = SourceType::Directory;
    config.content_type = "Binary".to_string(); // MDD files typically contain resources
    config.default_sorting_locale = "root".to_string();
    config.build_mdd = true;
    config.password = password.to_string(); 
        
    // Use ZDBBuilder to build the MDD file with DataDirLoader
    ZDBBuilder::build_with_config(&config, Some(print_progress))?;
    
    info!("MDD file created successfully: {}", output_file.display());
    
    Ok(())
}
