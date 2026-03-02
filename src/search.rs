use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use log::{error, info};

use mdx::readers::MdxReader;
use mdx::readers::ZdbReader;
use mdx::Result;

use crate::utils;

// 简化的搜索函数 - ZDB
pub fn search_zdb(
    file_path: &PathBuf,
    key: &str,
    preview: bool,
    start_with_match: bool,
    partial_match: bool,
) -> Result<()> {
    info!("Search {} in zdb file: {}", key, file_path.display());

    let file = File::open(file_path)?;
    let mut zdb = ZdbReader::<BufReader<File>>::from_reader(BufReader::new(file), "", "")?;

    info!(
        "Searching key: '{}' with start_with_match: {}, partial_match: {}",
        key, start_with_match, partial_match
    );

    match zdb.find_first_match(key, start_with_match, partial_match, true) {
        Ok(Some(index)) => {
            // Display main entry
            println!("Found: '{}' at {} ", index.key, index.entry_no);

            let indexes = zdb.get_indexes(index.entry_no, 10)?;
            for (i, entry_index) in indexes.iter().enumerate() {
                let content_bytes = zdb.get_data(entry_index, true)?;
                let content = if zdb.is_binary_content() {
                    format!("Binary content ({} bytes)", content_bytes.len())
                } else {
                    String::from_utf8_lossy(&content_bytes).to_string()
                };
                println!(
                    "  {}: '{}' at {} - Size: {} bytes",
                    i + 1,
                    entry_index.key,
                    entry_index.entry_no,
                    content.len()
                );
                if preview {
                    let text_content = mdx::utils::extract_text_from_html(&content)?;
                    println!("     Preview: {}", utils::take_chars(&text_content, 1000));
                }
            }
            Ok(())
        }
        Ok(None) => {
            info!("No match found for key: {}", key);
            Ok(())
        }
        Err(e) => {
            error!("Error with search: {:}", e);
            Err(e)
        }
    }
}

// 简化的搜索函数 - MDX Reader
pub fn search_mdx_db(
    file_path: &PathBuf,
    key: &str,
    preview: bool,
    start_with_match: bool,
    partial_match: bool,
) -> Result<()> {
    info!("Search {} in mdx file: {}", key, file_path.display());

    let mdx_url = url::Url::from_file_path(file_path)
        .map_err(|_| mdx::ZdbError::invalid_path(format!("{}", file_path.display())))?;

    let mut mdx_reader = MdxReader::from_url(&mdx_url, "")?;

    info!(
        "Searching key: '{}' with start_with_match: {}, partial_match: {}",
        key, start_with_match, partial_match
    );

    match mdx_reader.find_index(key, start_with_match, partial_match, true) {
        Ok(Some(key_index)) => {
            // Display main entry
            println!("Found: '{}' at {} ", key_index.key, key_index.entry_no);

            let indexes = mdx_reader.get_indexes(key_index.entry_no, 10)?;
            for (i, entry_index) in indexes.iter().enumerate() {
                let content = mdx_reader.get_html(entry_index)?;
                println!(
                    "  {}: '{}' at {} - Size: {} bytes",
                    i + 1,
                    entry_index.key,
                    entry_index.entry_no,
                    content.len()
                );
                if preview {
                    let text_content = mdx::utils::utils::html_to_text(&content);
                    println!("     Preview: {}", utils::take_chars(&text_content, 1000));
                }
            }
            Ok(())
        }
        Ok(None) => {
            info!("No match found for key: {}", key);
            Ok(())
        }
        Err(e) => {
            error!("Error with search: {:}", e);
            Err(e)
        }
    }
}

// Run function for search command
pub fn run_search(
    path: &str,
    keyword: &str,
    mdx_db: bool,
    preview: bool,
    start_with_match: bool,
    partial_match: bool,
) -> mdx::Result<()> {
    let target = mdx::utils::io_utils::fix_windows_path_buf(PathBuf::from(
        shellexpand::tilde(path).to_string(),
    ));
    if mdx_db {
        // Use mdx_db mode for search
        search_mdx_db(&target, keyword, preview, start_with_match, partial_match)?;
    } else {
        // Use zdb mode for search
        search_zdb(&target, keyword, preview, start_with_match, partial_match)?;
    }
    Ok(())
}
