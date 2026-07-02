//! # rocq2md
//!
//! A library for converting Rocq files to Markdown.
//!
//! ## Features
//!
//! - Converts top-level `(* ... *)` comments (starting at column 0, outside
//!   of any `Proof ... Qed`/`Defined`/`Admitted`/`Abort` environment) into
//!   plain Markdown text.
//! - Leaves all other Rocq source (including comments that were not
//!   converted) inside ` ```rocq ` code fences.
//! - Warns about comment blocks that were not converted because they did not
//!   start at the beginning of a line (comments inside proofs are silently
//!   left as-is).
//! - Supports a `--#-- ... --#--` marker pair to strip text out of a
//!   converted comment block, usable inline or across multiple lines.

mod rocq2md_core;

pub use rocq2md_core::{
    build_segments, convert_to_markdown, process_directory, process_file, ConversionReport,
    Segment, Warning,
};
