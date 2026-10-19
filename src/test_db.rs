use std::collections::{HashMap, LinkedList};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use log::{debug, error, info};
use rand::RngExt;
use regex::Regex;
use url::Url;

use mdx::Result;
use mdx::readers::MdxReader;
use mdx::readers::ZdbReader;
use mdx::storage::key_block::KeyIndex;
use mdx::storage::meta_unit::ContentType;
use mdx::utils::io_utils::scan_dir;
use mdx::utils::progress_report::ProgressState;
use mdx::utils::url_utils::get_decoded_path;

use crate::error_printer;
use crate::report::print_progress;
use crate::utils;

/// Constant-memory pseudo-random permutation generator for huge ranges,
/// combined with sparse Partial Fisher-Yates for small `sample_count`.
enum IndexSampler {
    Sequential {
        current: u64,
        remaining: u64,
    },
    /// Used when `sample_count` is reasonably small (<= 65_536):
    /// true uniform sampling without replacement using only O(sample_count) memory.
    SparseFisherYates {
        rng: rand::rngs::ThreadRng,
        swaps: HashMap<u64, u64>,
        step: u64,
        sample_count: u64,
        entry_count: u64,
    },
    /// Used when `sample_count` is large (or testing all entries randomly on a huge dict):
    /// O(1) memory cycle-walking bijective Feistel/LCG permutation over `[0, entry_count)`.
    BijectiveStream {
        state: u64,
        remaining: u64,
        entry_count: u64,
        mask: u64,
        key: u64,
    },
}

impl IndexSampler {
    fn new(entry_count: u64, sample_count: u64, random: bool) -> Self {
        if !random || entry_count <= 1 || sample_count == 0 {
            return Self::Sequential { current: 0, remaining: sample_count };
        }

        if sample_count <= 65_536 {
            Self::SparseFisherYates {
                rng: rand::rng(),
                swaps: HashMap::with_capacity(sample_count as usize),
                step: 0,
                sample_count,
                entry_count,
            }
        } else {
            let mut rng = rand::rng();
            let bits = (64 - (entry_count - 1).leading_zeros()).max(2);
            let mask = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
            let key: u64 = rng.random();
            Self::BijectiveStream { state: 0, remaining: sample_count, entry_count, mask, key }
        }
    }
}

impl Iterator for IndexSampler {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Sequential { current, remaining } => {
                if *remaining == 0 {
                    return None;
                }
                let val = *current;
                *current += 1;
                *remaining -= 1;
                Some(val)
            }
            Self::SparseFisherYates { rng, swaps, step, sample_count, entry_count } => {
                if *step >= *sample_count {
                    return None;
                }
                let i = *step;
                *step += 1;
                let j = rng.random_range(i..*entry_count);
                let val_i = swaps.remove(&i).unwrap_or(i);
                let val_j = if i == j { val_i } else { swaps.remove(&j).unwrap_or(j) };
                if i != j {
                    swaps.insert(j, val_i);
                }
                Some(val_j)
            }
            Self::BijectiveStream { state, remaining, entry_count, mask, key } => {
                if *remaining == 0 {
                    return None;
                }
                *remaining -= 1;
                loop {
                    let mut x = *state;
                    *state += 1;
                    // 4-round bijective mixing within power-of-two domain `mask`
                    for round in 0..4u64 {
                        x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15 | 1) & *mask;
                        x ^= (*key ^ round) & *mask;
                        x ^= x >> (mask.count_ones() / 2);
                    }
                    if x < *entry_count {
                        return Some(x);
                    }
                }
            }
        }
    }
}

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
    let entries_to_test = test_count.map_or(entry_count, |count| (count as u64).min(entry_count));

    info!(
        "Total entry count: {entry_count}, testing {entries_to_test} entries from 0 to {entry_count}"
    );

    let sampler = IndexSampler::new(entry_count, entries_to_test, random);

    // 创建进度报告
    let mut progress_report =
        ProgressState::new("dump_db", entries_to_test, 5, Some(print_progress));

    for (i, entry_no) in sampler.enumerate() {
        let (key_index, _content) = get_index_and_content(entry_no)?;
        debug!("Entry[{}] (original: {}): {}", i, entry_no, key_index.key);
        progress_report.report((i as u64) + 1);
    }
    info!("Test finished\n");
    Ok(())
}

// test_mdx 函数 - 包含文件打开和数据库初始化逻辑
pub fn test_mdx(file_path: &Path, test_count: Option<usize>, random: bool) -> Result<()> {
    let abs_path = utils::to_absolute_path(file_path)?;
    let mdx_url = Url::from_file_path(&abs_path)
        .map_err(|()| mdx::ZdbError::invalid_path(format!("{}", abs_path.display())))?;

    println!("Opening mdx db: {}", file_path.display());
    let mut mdx_reader = MdxReader::from_url(&mdx_url, "")?;

    debug!("Raw header xml: {}", mdx_reader.content_db.meta.raw_header_xml);
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
pub fn test_zdb(file_path: &Path, test_count: Option<usize>, random: bool) -> Result<()> {
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
        let abs_target = utils::to_absolute_path(&target)?;
        let dir_url = Url::from_directory_path(&abs_target)
            .map_err(|()| mdx::ZdbError::invalid_path(format!("{}", abs_target.display())))?;
        let target_path = mdx::utils::io_utils::fix_windows_path_buf(get_decoded_path(&dir_url)?);

        // Scan an MDX file (case-insensitive)
        let pattern_mdx = Regex::new(r"(?i).*\.mdx$").map_err(|e| {
            mdx::ZdbError::invalid_path(format!("failed to compile mdx pattern: {e}"))
        })?;
        scan_dir(&target_path, &pattern_mdx, false, &mut files)?;
        println!("Found {} '.mdx' files", files.len());

        if !mdx_mode {
            // Scan an MDD file (case-insensitive)
            let pattern_mdd = Regex::new(r"(?i).*\.mdd$").map_err(|e| {
                mdx::ZdbError::invalid_path(format!("failed to compile mdd pattern: {e}"))
            })?;
            let mut mdd_files = LinkedList::<PathBuf>::new();
            scan_dir(&target_path, &pattern_mdd, false, &mut mdd_files)?;
            println!("Found {} '.mdd' files", mdd_files.len());
            files.append(&mut mdd_files);
        }
        if files.is_empty() {
            println!("No .mdx or .mdd files found in directory: {}", target_path.display());
            return Ok(());
        }
    } else {
        files.push_back(target.clone());
    }

    let mut first_error: Option<mdx::ZdbError> = None;

    for file_path in files {
        let result = if mdx_mode {
            test_mdx(&file_path, test_count, random)
        } else {
            test_zdb(&file_path, test_count, random)
        };
        if let Err(e) = result {
            if first_error.is_none() {
                first_error = Some(e);
            } else {
                error!("{}", error_printer::format_error(&e));
            }
        }
    }

    if let Some(err) = first_error {
        return Err(err);
    }

    Ok(())
}
