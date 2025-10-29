use clap::{Parser, Subcommand};
use log::{error, LevelFilter};

use mdx::Result;

use crate::fts_index::run_create_index;
use crate::search::run_search;
use crate::test_db::run_test_db;

mod error_printer;
mod test_db;
mod search;
mod fts_index;
mod utils;
mod keygen;
mod convert_db;
mod build_mdd;
mod report;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "mdx_util",
    author = "Rayman Zhang",
    version = "1.0.0",
    about = "A comprehensive testing tool for MDX/ZDB database files",
    long_about = "mdx_util is a command-line tool for testing and analyzing MDX and ZDB database files. 
It supports database dumping, keyword searching, index creation, and full-text search capabilities.",
    after_help = "Examples:
  mdx_util test --mode mdx --count 100 /path/to/mdx/files
  mdx_util convert-db /path/to/config.json
  mdx_util search /path/to/file.mdx \"keyword\"
  mdx_util create-index /path/to/file.mdx
  mdx_util fts-search /path/to/file.mdx \"search term\" "
)]
struct Args {
    /// Log level (error, warn, info, debug, trace)
    #[arg(long, value_name = "LEVEL", hide = true)]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Test mdx/mdd file structure by opening and reading contents
    #[command(
        name = "test",
        about = "Verify mdx/mdd file format by opening it and reading its contents",
        after_help = "Examples:
  mdx_util test /path/to/file.mdx
  mdx_util test --count 100 --random /path/to/directory
  mdx_util test --mode mdx --count 50 /path/to/file.mdx"
    )]
    Test {
        /// Path to directory or file containing MDX/ZDB files
        #[arg(value_name = "DIR|FILE")]
        path: String,

        /// Mode for testing (zdb or mdx)
        #[arg(long, value_name = "MODE", default_value = "zdb", help = "Testing mode: zdb (single file) or mdx (mdx+mdd group)")]
        mode: String,

        /// Number of entries to test (default: all entries)
        #[arg(long, value_name = "COUNT", help = "Limit the number of entries to test")]
        count: Option<usize>,

        /// Randomly sample entries instead of sequential reading
        #[arg(long, help = "Randomly sample entries instead of reading sequentially")]
        random: bool,
    },

    /// Convert file to mdx format
    #[command(
        name = "convert-db",
        about = "Convert specified file to mdx format",
        after_help = "Examples:
  mdx_util convert-db /path/to/config.json"
    )]
    ConvertDb {
        /// Path to file to convert
        #[arg(value_name = "FILE")]
        file: String,

        /// Generate an example config file only
        #[arg(long, default_value = "false", help = "Generate an example config file only")]
        generate_config_only: bool,
    },

    /// Build MDD from directory
    #[command(
        name = "build-mdd",
        about = "Pack files in directory into MDD format",
        after_help = "Examples:
  mdx_util build-mdd /path/to/directory"
    )]
    BuildMdd {
        /// Path to directory containing files to pack
        #[arg(value_name = "DIR")]
        directory: String,

        /// The target mdd file to build
        #[arg(value_name = "FILE")]
        file: String,

        /// The password to use for the mdd file
        #[arg(value_name = "PASSWORD")]
        password: String,
    },

    /// Search entries in file and display up to 10 subsequent entries
    #[command(
        name = "list",
        about = "Search for keywords in database files and list entries",
        after_help = "Examples:
  mdx_util list /path/to/file.mdx \"keyword\"
  mdx_util list --mode mdx --preview /path/to/file.mdx \"search term\""
    )]
    List {
        /// Path to file containing MDX/ZDB data
        #[arg(value_name = "FILE")]
        file: String,

        /// Keyword to search for in the database
        #[arg(value_name = "KEYWORD")]
        keyword: String,

        /// Mode for searching (zdb or mdx)
        #[arg(long, value_name = "MODE", default_value = "zdb", help = "Search mode: zdb or mdx")]
        mode: String,

        /// Show HTML->text preview of content
        #[arg(long, help = "Show HTML content converted to text preview")]
        preview: bool,

        #[arg(long, help="Start with match")]
        start_with_match: bool,

        #[arg(long, help="Partial match")]
        partial_match: bool,
    },  

    /// Create full-text search index for mdx file
    #[command(
        name = "create-index",
        about = "Create full-text search index (.mdi) for specified mdx file",
        after_help = "Examples:
  mdx_util create-index /path/to/file.mdx"
    )]
    CreateIndex {
        /// Path to MDX file (must be a single .mdx file, not a directory)
        #[arg(value_name = "MDX_FILE", help = "Path to the MDX file to index")]
        file: String,
    },

    /// Generate authorization code from password and id
    #[command(
        name = "keygen",
        about = "Generate authorization code from password and id",
        after_help = "Examples:
  mdx_util keygen P@ssw0rd user@123.com
  mdx_util keygen P@ssw0rd 006F0050-0063-006B-0065-4444556494345454D00"
    )]
    Keygen {
        /// Password for key generation
        #[arg(value_name = "PASSWORD")]
        password: String,

        /// ID for key generation
        #[arg(value_name = "ID")]
        id: String,
    },
    
    /// Perform full-text search using Tantivy index
    #[command(
        name = "fts-search",
        about = "Perform full-text search using Tantivy index",
        after_help = "Examples:
  mdx_util fts-search /path/to/file.mdx \"search term\"
  mdx_util fts-search /path/to/file.mdx \"exact phrase\""
    )]
    FtsSearch {
        /// Path to MDX file
        #[arg(value_name = "MDX_FILE", help = "Path to the MDX file to search in")]
        mdx_file: String,

        /// Keyword to search for in the index
        #[arg(value_name = "KEYWORD")]
        keyword: String,
    },
}

fn main() {
    unsafe { std::env::set_var("RUST_BACKTRACE", "1"); }
    let args = Args::parse();

    let log_level = if let Some(log_level) = &args.log_level {
        log_level.to_lowercase()
    } else {
        std::env::var("RUST_LOG").unwrap_or("info".to_string()).to_lowercase()
    };
    let level_filter= match log_level.as_str() {
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,
            "debug" => LevelFilter::Debug,
            "trace" => LevelFilter::Trace,
            _ => LevelFilter::Info, // 默认级别
        };
    fern::Dispatch::new()
    .format(|out, message, _record| {
        out.finish(format_args!(
            "{}",
            message
        ))
    })
    .level(level_filter)
    .level_for("tantivy", LevelFilter::Warn)
    .chain(std::io::stdout())
    .apply().unwrap();
    if let Err(e) = run(&args) {
        error!("{}", error_printer::format_error(&e));
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<()> {
    match &args.command {
        Commands::Test { path, mode, count, random } => {
            let mdx_mode = mode == "mdx";
            run_test_db(path, mdx_mode, *count, *random)
        },
        Commands::ConvertDb { file, generate_config_only } => {
            convert_db::run_convert_db(file, *generate_config_only)
        },
        Commands::BuildMdd { directory, file, password } => {
            build_mdd::run_build_mdd(directory, &password, &file)
        },
        Commands::List { file, keyword, mode, preview, start_with_match, partial_match } => {
            let mdx_mode = mode == "mdx";
            run_search(file, keyword, mdx_mode, *preview, *start_with_match, *partial_match)
        },
        Commands::CreateIndex { file } => {
            run_create_index(file)
        },
        Commands::Keygen { password, id } => {
            keygen::run_keygen(password, id)
        },
        Commands::FtsSearch { mdx_file, keyword } => {
            fts_index::run_fulltext_search(mdx_file, keyword, 100)
        },
    }
}
