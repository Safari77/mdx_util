use log::info;
use serde_json;

use mdx::builder::zdb_builder::ZDBBuilder;
use mdx::builder::BuilderConfig;
use mdx::{Result, ZdbError};

use crate::report::print_progress;

pub fn generate_config_file(file_path: &str) -> Result<()> {
    let config_path = shellexpand::tilde(file_path).to_string();
    let mut config = BuilderConfig::default();
    config.input_path = "/path/to/input.txt".to_string();
    config.output_file = "/path/to/output.mdx".to_string();
    config.build_mdd = false;

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| ZdbError::general_error(e.to_string()))?;
    std::fs::write(config_path, json)?;
    Ok(())
}

pub fn run_convert_db(config_file_path: &str, generate_config_only: bool) -> Result<()> {
    if generate_config_only {
        generate_config_file(config_file_path)?;
        return Ok(());
    }

    info!("Converting file: {}", config_file_path);

    let mdx_path = shellexpand::tilde(config_file_path).to_string();

    let config: BuilderConfig = serde_json::from_str(&std::fs::read_to_string(mdx_path.clone())?)
        .map_err(|e| ZdbError::general_error(e.to_string()))?;

    // config.input_file = mdx_path.clone();
    // config.output_file = mdx_path.clone().replace(".json", ".new.mdx");
    // config.build_mdd = false;

    info!("Input file: {}", config.input_path);
    info!("Output file: {}", config.output_file);

    ZDBBuilder::build_with_config(&config, Some(print_progress))?;

    info!("Conversion completed successfully!");
    Ok(())
}
