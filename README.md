# mdx_util - MDX/MDD Database Utility Tool

A comprehensive command-line tool for working with MDict format `.mdx` and `.mdd` files. This tool provides powerful capabilities for testing, searching, indexing, and converting MDict database files.

## Overview

`mdx_util` is designed to work in conjunction with the [mdx](https://github.com/raymanzhang/mdx) library, providing convenient CLI access to MDict format functionality. It supports both `.mdx` files (text/dictionary entries) and `.mdd` files (binary resources/media), enabling dictionary developers and users to extract, search, and process dictionary data efficiently.

## Features

### 📋 Core Capabilities

- **Database Testing & Validation**: Verify MDX/MDD file integrity by reading and parsing contents
- **Keyword Search**: Find entries by keyword with optional fuzzy matching and HTML preview
- **Full-Text Search (FTS)**: Build and query Tantivy-based full-text indexes for fast searches
- **Database Conversion**: Convert various formats to MDX format with configuration files
- **Resource Packaging**: Build MDD files from directories containing media/resource files
- **Authorization**: Generate encryption keys and registration codes for password-protected dictionaries

### 🎯 Commands

#### `test` - Verify Database Structure
Test and validate MDX/MDD database files:
```bash
# Test a single MDX file
mdx_util test /path/to/file.mdx

# Test all MDX files in a directory with limit
mdx_util test --mode mdx --count 100 /path/to/directory

# Test MDD files (binary content)
mdx_util test --mode zdb --count 50 /path/to/file.mdd

# Random sampling instead of sequential reading
mdx_util test --random /path/to/file.mdx
```

**Options**:
- `--mode`: `zdb` (default) or `mdx` - Testing mode for file type
- `--count`: Limit number of entries to test (default: all)
- `--random`: Randomly sample entries instead of sequential reading

#### `list` - Search Database Entries
Search for keywords and display matching entries with surrounding context:
```bash
# Basic keyword search
mdx_util list /path/to/file.mdx "apple"

# Search with HTML-to-text preview
mdx_util list --preview /path/to/file.mdx "search term"

# Exact prefix match
mdx_util list --start-with-match /path/to/file.mdx "prefix"

# Partial match
mdx_util list --partial-match /path/to/file.mdx "pattern"
```

**Options**:
- `--mode`: `zdb` (default) or `mdx` - Search mode
- `--preview`: Show HTML content converted to text preview
- `--start-with-match`: Match entries starting with keyword (prefix search)
- `--partial-match`: Allow partial matching

#### `create-index` - Build Full-Text Search Index
Create a Tantivy-based full-text search index for an MDX file:
```bash
mdx_util create-index /path/to/file.mdx
```

This generates an `.mdi` index file that enables fast full-text search capabilities.

#### `fts-search` - Full-Text Search
Perform fast full-text searches using the generated index:
```bash
mdx_util fts-search /path/to/file.mdx "search term"

# Search with exact phrase
mdx_util fts-search /path/to/file.mdx "exact phrase"
```

Returns top 100 results with relevance scores.

#### `convert-db` - Convert to MDX Format
Convert source files to MDX format using a configuration file:
```bash
# Generate example configuration file
mdx_util convert-db config.json --generate-config-only

# Convert using existing configuration
mdx_util convert-db config.json
```

Configuration file format (JSON):
```json
{
  "input_path": "/path/to/source.txt",
  "output_file": "/path/to/output.mdx",
  "data_source_format": "TextFile",
  "content_type": "Html",
  "default_sorting_locale": "en_US",
  "build_mdd": false,
  "password": ""
}
```

#### `build-mdd` - Create MDD Resource Package
Pack files from a directory into MDD format:
```bash
mdx_util build-mdd /path/to/resources output.mdd "password"
```

Useful for creating dictionary resource files containing images, audio, and other media.

#### `keygen` - Generate Authorization Keys
Generate encryption keys and registration codes for password-protected dictionaries:
```bash
mdx_util keygen "password" "user@example.com"

# With UUID identifier
mdx_util keygen "password" "006F0050-0063-006B-0065-4444556494345454D00"
```

Output:
```
Password: password
DB cipher: [hex_encoded_cipher]
Identified by: user@example.com
Reg code for end user: [hex_encoded_reg_code]
```

## Installation

### Prerequisites
- Rust 1.70+ (with Cargo)
- The [mdx](https://github.com/raymanzhang/mdx) library (as a dependency)

### Build from Source
```bash
cargo build --release
```

The compiled binary will be available at `target/release/mdx_util`.

## Configuration

### Logging
Control logging verbosity using environment variables or CLI flags:
```bash
# Via environment variable
RUST_LOG=debug mdx_util test /path/to/file.mdx

# Via CLI flag
mdx_util --log-level debug test /path/to/file.mdx
```

Supported log levels: `error`, `warn`, `info` (default), `debug`, `trace`

### Path Expansion
All paths support shell expansion (`~` for home directory):
```bash
mdx_util test ~/dictionaries/my-dict.mdx
```

## Dependencies

Core dependencies:
- **mdx**: MDict format reader/writer library
- **tantivy**: Full-text search engine for FTS capabilities
- **clap**: Command-line argument parsing
- **serde/serde_json**: JSON configuration parsing
- **snafu**: Error handling with context
- **lol_html**: HTML processing for content extraction
- **zip**: Archive handling for MDD packaging
- **rand**: Random sampling for test mode
- **log/fern**: Logging infrastructure

See [Cargo.toml](./Cargo.toml) for complete dependency list and versions.

## Use Cases

1. **Dictionary Development**: Test and validate dictionary files during creation
2. **Quality Assurance**: Verify database integrity and encoding
3. **Content Migration**: Convert from other formats to MDX format
4. **Search Optimization**: Build full-text indexes for faster lookups
5. **Resource Management**: Package media files into MDD format
6. **Authentication**: Generate keys for encrypted dictionaries

## Examples

### Complete Workflow
```bash
# 1. Convert source data to MDX
mdx_util convert-db config.json

# 2. Create full-text search index
mdx_util create-index output.mdx

# 3. Test the database
mdx_util test output.mdx --count 100

# 4. Search for content
mdx_util list output.mdx "example" --preview

# 5. Perform full-text search
mdx_util fts-search output.mdx "search term"
```

### Dictionary with Resources
```bash
# 1. Create resource package
mdx_util build-mdd ~/images dict.mdd "secret_password"

# 2. Package with dictionary
# (Dictionary content and resources are linked via path references)

# 3. Test both files
mdx_util test output.mdx
mdx_util test dict.mdd --mode zdb
```

## Error Handling

The tool provides detailed error messages with context information. Common issues:
- **Invalid file path**: Ensure file exists and path is correct
- **Corrupted database**: File may be damaged or incompatible
- **Missing index**: Create index before performing FTS search
- **Permission denied**: Check file access permissions

## License

GNU Affero General Public License v3.0 (AGPL-3.0)

## Author

Rayman Zhang

## Contributing

For issues or contributions, refer to the main [mdx](https://github.com/raymanzhang/mdx) project repository.
