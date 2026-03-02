use std::cmp::min;
use std::collections::LinkedList;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use log::{debug, error, info};
use rand::seq::SliceRandom;
use regex::Regex;
use url::Url;

use mdx::readers::MdxReader;
use mdx::readers::ZdbReader;
use mdx::storage::key_block::KeyIndex;
use mdx::storage::meta_unit::ContentType;
use mdx::utils::io_utils::scan_dir;
use mdx::utils::progress_report::ProgressState;
use mdx::utils::url_utils::get_decoded_path;
use mdx::Result;

use crate::error_printer;
use crate::report::print_progress;

// 统一的 dump_db 函数，根据数据源类型和内容类型来决定输出
pub fn dump_db_unified<F>(
    entry_count: u64,
    test_count: Option<usize>,
    random: bool,
    mut get_index_and_content: F,
    _is_binary: bool,
) -> Result<()>
where
    F: FnMut(u64) -> Result<(KeyIndex, String)>,
{
    let entries_to_test = if let Some(count) = test_count {
        min(count, entry_count as usize)
    } else {
        entry_count as usize
    };

    info!(
        "Total entry count: {}, testing {} entries from 0 to {}",
        entry_count, entries_to_test, entry_count
    );

    let mut indices: Vec<u64> = (0..entry_count).collect();

    if random {
        indices.shuffle(&mut rand::rng());
    }

    indices.truncate(entries_to_test);

    // 创建进度报告
    let mut progress_report =
        ProgressState::new("dump_db", entries_to_test as u64, 5, Some(print_progress));

    for (i, &entry_no) in indices.iter().enumerate() {
        let (key_index, _content) = get_index_and_content(entry_no)?;
        debug!("Entry[{}] (original: {}): {}", i, entry_no, key_index.key);
        progress_report.report(i as u64);
    }
    info!("Test finished\n");
    Ok(())
}

// test_mdx 函数 - 包含文件打开和数据库初始化逻辑
pub fn test_mdx(file_path: &PathBuf, test_count: Option<usize>, random: bool) -> Result<()> {
    let mdx_url = Url::from_file_path(file_path)
        .map_err(|_| mdx::ZdbError::invalid_path(format!("{}", file_path.display())))?;

    println!("Opening mdx db: {}", file_path.display());
    let mut mdx_reader = MdxReader::from_url(&mdx_url, "")?;

    debug!(
        "Raw header xml: {}",
        mdx_reader.content_db.meta.raw_header_xml
    );
    debug!("Db info: {:?}", mdx_reader.content_db.meta.db_info);

    let entry_count = mdx_reader.get_entry_count();
    let is_binary = {
        let content_type = mdx_reader.content_db.meta.db_info.content_type.clone();
        matches!(content_type, ContentType::Binary)
    };

    dump_db_unified(
        entry_count,
        test_count,
        random,
        |entry_no| {
            let key_index = mdx_reader.get_index(entry_no as mdx::storage::key_block::EntryNo)?;
            let content = mdx_reader.get_html(&key_index)?;
            Ok((key_index, content))
        },
        is_binary,
    )
}

// dump_zdb 函数 - 包含文件打开和数据库初始化逻辑
pub fn test_zdb(file_path: &PathBuf, test_count: Option<usize>, random: bool) -> Result<()> {
    println!("Opening zdb db: {}", file_path.display());
    let file = File::open(file_path)?;

    let mut zdb = ZdbReader::<BufReader<File>>::from_reader(BufReader::new(file), "", "")?;
    debug!("Raw header xml: {}", zdb.meta.raw_header_xml);
    debug!("Db info: {:?}", zdb.meta.db_info);

    let entry_count = zdb.get_entry_count();
    let is_binary = zdb.meta.db_info.is_mdd;

    dump_db_unified(
        entry_count,
        test_count,
        random,
        |entry_no| {
            let key_index = zdb.get_index(entry_no as mdx::storage::key_block::EntryNo)?;
            let content_bytes = zdb.get_data(&key_index, true)?;
            let content_str = if zdb.meta.db_info.is_mdd {
                format!("[Binary data, length: {}]", content_bytes.len())
            } else {
                String::from_utf8_lossy(&content_bytes).to_string()
            };
            Ok((key_index, content_str))
        },
        is_binary,
    )
}

// Run function for dump-db command
pub fn run_test_db(
    path: &str,
    mdx_mode: bool,
    test_count: Option<usize>,
    random: bool,
) -> mdx::Result<()> {
    let target = mdx::utils::io_utils::fix_windows_path_buf(PathBuf::from(
        shellexpand::tilde(path).to_string(),
    ));
    let mut files = LinkedList::<PathBuf>::new();

    if target.is_dir() {
        println!("Check Directory: {}", target.display());
        let dir_url = Url::from_directory_path(&target)
            .map_err(|_| mdx::ZdbError::invalid_path(format!("{}", &target.display())))?;
        let target_path = mdx::utils::io_utils::fix_windows_path_buf(get_decoded_path(&dir_url)?);

        // 扫描mdx文件
        let pattern_mdx = Regex::new(r".*\.mdx$").unwrap();
        scan_dir(&target_path, &pattern_mdx, false, &mut files)?;
        println!("Found {} '.mdx' files", files.len());

        if !mdx_mode {
            // 扫描mdd文件
            let pattern_mdd = Regex::new(r".*\.mdd$").unwrap();
            let mut mdd_files = LinkedList::<PathBuf>::new();
            scan_dir(&target_path, &pattern_mdd, false, &mut mdd_files)?;
            println!("Found {} '.mdd' files", mdd_files.len());
            files.append(&mut mdd_files);
        }
        if files.is_empty() {
            println!(
                "No .mdx or .mdd files found in directory: {}",
                target_path.display()
            );
            return Ok(());
        }
    } else {
        files.push_back(target.clone());
    }

    for file_path in files {
        let result = if mdx_mode {
            test_mdx(&file_path, test_count, random)
        } else {
            test_zdb(&file_path, test_count, random)
        };
        if let Err(e) = result {
            error!("{}", error_printer::format_error(&e));
        }
    }
    Ok(())
}
