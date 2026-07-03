//! # rocq2md
//!
//! A library for converting Rocq files to Markdown.
//!
//! ## Features
//!
//! - Converts top-level Rocq comments not contained in proofs into Markdown text.
//! - Converts Rocq code into Markdown code blocks.
//! - Supports special markers to prevent parts of a comment from being converted to Markdown.
//! - Warns about comment blocks that were not converted.
//! - Generates a log file to report on warnings.

mod rocq2md_core;

pub use rocq2md_core::{
    build_segments, convert_to_markdown, process_directory, process_file, ConversionReport,
    Segment, Warning,
};
