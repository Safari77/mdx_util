use std::path::PathBuf;

use log::{error, info};

use mdx::readers::MdxReader;
use mdx::Result;

use crate::report::print_progress;
use crate::utils;

// Full-text search function using MdxReader with FTS index
pub fn search_mdx_fulltext(file_path: &PathBuf, query: &str, max_results: usize) -> Result<()> {
    info!(
        "Full-text search for '{}' in MDX file: {}",
        query,
        file_path.display()
    );

    // Create URL from file path and open with MdxReader
    let mdx_url = url::Url::from_file_path(file_path)
        .map_err(|_| mdx::ZdbError::invalid_path(format!("{}", file_path.display())))?;

    let mut mdx_reader = MdxReader::from_url(&mdx_url, "")?;

    // Check if FTS is available
    if !mdx_reader.is_fts_available() {
        println!("Full-text search index is not available for this MDX file.");
        println!(
            "Please create an index first using: mdx_util create-index {}",
            file_path.display()
        );
        return Ok(());
    }

    info!("✓ FTS index is available, performing search...");

    // Perform full-text search
    let search_results = mdx_reader.fts_search(query, max_results)?;

    // Display search results
    println!("\n=== Full-Text Search Results for '{}' ===\n", query);

    if search_results.is_empty() {
        println!("No results found for query: '{}'", query);
        return Ok(());
    }

    for (index, (score, entry_no, key)) in search_results.iter().enumerate() {
        println!("Result #{} (Score: {:.3})", index + 1, score);
        println!("Entry No: {}", entry_no);
        println!("Key: {}", key);

        // Get the original HTML content from the MDX file
        if let Ok(key_index) = mdx_reader.get_index(*entry_no as mdx::storage::key_block::EntryNo) {
            match mdx_reader.get_html(&key_index) {
                Ok(html_content) => {
                    // Extract text content and truncate for display
                    let text_content = mdx::utils::extract_text_from_html(&html_content)?;
                    println!(
                        "Content Preview: {}",
                        utils::take_chars(&text_content, 1024)
                    );
                }
                Err(_) => {
                    println!("Content: [Error retrieving content]");
                }
            }
        } else {
            println!("Content: [Error retrieving entry]");
        }

        println!("{}", "-".repeat(80));
        println!();
    }

    println!("Total results: {}", search_results.len());

    Ok(())
}

// Run function for create-index command
pub fn run_create_index(mdx_file_path: &str) -> mdx::Result<()> {
    use log::*;
    use std::path::PathBuf;

    let target = mdx::utils::io_utils::fix_windows_path_buf(PathBuf::from(
        shellexpand::tilde(mdx_file_path).to_string(),
    ));

    // Check if the target is a file
    if !target.is_file() {
        error!("Path must be an MDX file: {}", target.display());
        return Err(mdx::ZdbError::invalid_path(format!(
            "Not a file: {}",
            target.display()
        )));
    }

    // Check if it's an MDX file
    if target.extension().and_then(|s| s.to_str()) != Some("mdx") {
        error!("File must have .mdx extension: {}", target.display());
        return Err(mdx::ZdbError::invalid_path(format!(
            "Invalid file extension: {}",
            target.display()
        )));
    }

    info!("Creating index for MDX file: {}", target.display());

    // Create Tantivy index for the MDX file (includes merge and pack operations)
    mdx::builder::make_index(&target, Some(print_progress))?;

    Ok(())
}

// Run function for full-text search command
pub fn run_fulltext_search(path: &str, query: &str, max_results: usize) -> mdx::Result<()> {
    let target = mdx::utils::io_utils::fix_windows_path_buf(PathBuf::from(
        shellexpand::tilde(path).to_string(),
    ));

    if !target.is_file() {
        error!(
            "Path must be an MDX file for full-text search: {}",
            target.display()
        );
        return Ok(());
    }

    // Check if it's an MDX file
    if target.extension().and_then(|s| s.to_str()) != Some("mdx") {
        error!("File must have .mdx extension: {}", target.display());
        return Ok(());
    }

    search_mdx_fulltext(&target, query, max_results)
}
