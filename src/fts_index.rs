use std::path::{Path, PathBuf};

use log::{error, info};

use mdx::Result;
use mdx::readers::MdxReader;

use crate::report::print_progress;
use crate::safe_println;
use crate::utils;

// Full-text search function using MdxReader with FTS index
pub fn search_mdx_fulltext(
    file_path: &Path,
    query: &str,
    max_results: usize,
    quiet: bool,
    render: bool,
) -> Result<()> {
    if !quiet {
        info!("Full-text search for '{}' in MDX file: {}", query, file_path.display());
    }

    if max_results == 0 {
        if !quiet {
            safe_println!("Total results: 0");
        }
        return Ok(());
    }

    let abs_path = utils::to_absolute_path(file_path)?;

    // Create URL from absolute file path and open with MdxReader
    let mdx_url = url::Url::from_file_path(&abs_path)
        .map_err(|()| mdx::ZdbError::invalid_path(format!("{}", abs_path.display())))?;

    let mut mdx_reader = MdxReader::from_url(&mdx_url, "")?;

    // Check if FTS is available
    if !mdx_reader.is_fts_available() {
        if !quiet {
            safe_println!("Full-text search index is not available for this MDX file.");
            safe_println!(
                "Please create an index first using: mdx_util create-index {}",
                file_path.display()
            );
        }
        return Ok(());
    }

    if !quiet {
        info!("FTS index is available, performing search...");
    }

    // Fetch at least 100 results (to help locate an exact match) or `max_results` if larger
    let fetch_limit = max_results.max(100);
    let mut initial_results = mdx_reader.fts_search(query, fetch_limit)?;

    if !quiet {
        safe_println!("\n=== Full-Text Search Results for '{query}' ===\n");
    }

    if initial_results.is_empty() {
        if !quiet {
            safe_println!("No results found for query: '{query}'");
        }
        return Ok(());
    }

    // Find the index of the exact match
    let mut exact_match_idx = None;
    for (i, (_, _, key)) in initial_results.iter().enumerate() {
        if key == query {
            exact_match_idx = Some(i);
            break;
        }
    }

    // Build the final results list, placing the exact match first
    let mut search_results = Vec::with_capacity(max_results.min(initial_results.len()));

    // 1. Add the exact match first (if found and max_results > 0)
    if let Some(idx) = exact_match_idx
        && search_results.len() < max_results
    {
        search_results.push(initial_results.remove(idx));
    }

    // 2. Add the remaining results until we hit max_results
    for res in initial_results {
        if search_results.len() < max_results {
            search_results.push(res);
        } else {
            break;
        }
    }

    // Iterate over our newly ordered and truncated search_results
    for (index, (score, entry_no, key)) in search_results.iter().enumerate() {
        if quiet {
            safe_println!("\x1b[32m{key}\x1b[0m"); // Green
        } else {
            safe_println!("Result #{} (Score: {:.3})", index + 1, score);
            safe_println!("Entry No: {entry_no}");
            safe_println!("Key: {key}");
        }

        // Get the original HTML content from the MDX file
        if let Ok(key_index) = mdx_reader.get_index(*entry_no as mdx::storage::key_block::EntryNo) {
            match mdx_reader.get_html(&key_index) {
                Ok(html_content) => {
                    let preview_text = if render {
                        let rendered = utils::render_html_to_terminal(&html_content);
                        utils::truncate_ansi_safe(&rendered, 1_048_576)
                    } else {
                        utils::take_chars(&html_content, 1_048_576).to_string()
                    };

                    if quiet {
                        safe_println!("{preview_text}");
                    } else {
                        safe_println!("Content Preview:\n{preview_text}");
                    }
                }
                Err(_) => {
                    if !quiet {
                        safe_println!("Content: [Error retrieving content]");
                    }
                }
            }
        } else if !quiet {
            safe_println!("Content: [Error retrieving entry]");
        }

        safe_println!("{}", "—".repeat(80));
    }

    if !quiet {
        safe_println!("Total results: {}", search_results.len());
    }

    Ok(())
}

// Run function for create-index command
pub fn run_create_index(mdx_file_path: &str) -> mdx::Result<()> {
    let target = mdx::utils::io_utils::fix_windows_path_buf(PathBuf::from(
        shellexpand::tilde(mdx_file_path).to_string(),
    ));

    // Check if the target is a file
    if !target.is_file() {
        error!("Path must be an MDX file: {}", target.display());
        return Err(mdx::ZdbError::invalid_path(format!("Not a file: {}", target.display())));
    }

    // Check if it's an MDX file (case-insensitive)
    if !target
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mdx"))
    {
        error!("File must have .mdx extension: {}", target.display());
        return Err(mdx::ZdbError::invalid_path(format!(
            "Invalid file extension: {}",
            target.display()
        )));
    }

    let abs_target = utils::to_absolute_path(&target)?;
    info!("Creating index for MDX file: {}", abs_target.display());

    // Create Tantivy index for the MDX file (includes merge and pack operations)
    mdx::builder::make_index(&abs_target, Some(print_progress))?;

    Ok(())
}

// Run function for full-text search command
pub fn run_fulltext_search(
    path: &str,
    query: &str,
    max_results: usize,
    quiet: bool,
    render: bool,
) -> mdx::Result<()> {
    let target = mdx::utils::io_utils::fix_windows_path_buf(PathBuf::from(
        shellexpand::tilde(path).to_string(),
    ));

    if !target.is_file() {
        error!("Path must be an MDX file for full-text search: {}", target.display());
        return Err(mdx::ZdbError::invalid_path(format!("Not a file: {}", target.display())));
    }

    // Check if it's an MDX file (case-insensitive)
    if !target
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mdx"))
    {
        error!("File must have .mdx extension: {}", target.display());
        return Err(mdx::ZdbError::invalid_path(format!(
            "Invalid file extension: {}",
            target.display()
        )));
    }

    search_mdx_fulltext(&target, query, max_results, quiet, render)
}
