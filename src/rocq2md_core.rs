use std::fs;
use std::path::{Path, PathBuf};

/// A piece of the output: either plain Markdown text (extracted from a
/// converted comment block) or a chunk of raw Rocq source that should be
/// rendered inside a ```rocq code fence.
#[derive(Debug, PartialEq)]
pub enum Segment {
    Markdown(String),
    Code(String),
}

/// A single anomaly found while parsing a Rocq source file.
#[derive(Debug, PartialEq)]
pub enum Warning {
    /// A `(* ... *)` comment was left as code because it did not start at
    /// the beginning of a line.
    CommentNotAtLineStart { line: usize },
    /// A comment was still open at the end of the file.
    UnclosedComment,
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Warning::CommentNotAtLineStart { line } => write!(
                f,
                "line {}: comment block not converted to markdown because `(*` does not start at the beginning of the line",
                line
            ),
            Warning::UnclosedComment => {
                write!(f, "unclosed comment block at the end of the file")
            }
        }
    }
}

/// The result of parsing/converting a single piece of Rocq source: any
/// warnings encountered, plus the number of `--#-- ... --#--` blocks that
/// were stripped from the output (informational only, not a warning).
#[derive(Debug, Default, PartialEq)]
pub struct ConversionReport {
    pub warnings: Vec<Warning>,
    pub stripped_blocks: usize,
}

/// Tracks the small amount of state needed to decide whether a `(* ... *)`
/// comment should be converted to Markdown.
struct ParserState {
    /// Nesting depth of `(* ... *)` comments. 0 means we are in regular code.
    depth: usize,
    /// Whether the currently open top-level comment block is being converted
    /// to Markdown (only meaningful while `depth >= 1`).
    convert: bool,
    /// Whether we are currently inside a `Proof ... Qed/Defined/Admitted/Abort`
    /// environment (tracked via simple keyword counting on the code text).
    proof_depth: usize,
    /// Whether the `--#--` strip marker is currently "open" while accumulating
    /// Markdown or code text.
    strip_mode: bool,
    /// Whether there was blank space (space/tab) immediately before the
    /// currently-open `--#--` marker, so a single space can be reinserted
    /// once the matching closing marker is found.
    strip_had_space_before: bool,
    /// 1-indexed line number, used for warning messages.
    line_no: usize,
}

impl ParserState {
    fn new() -> Self {
        ParserState {
            depth: 0,
            convert: false,
            proof_depth: 0,
            strip_mode: false,
            strip_had_space_before: false,
            line_no: 1,
        }
    }
}

/// Checks an accumulated identifier-like word against the Rocq proof
/// keywords and updates the proof-nesting counter accordingly.
fn check_word(word_buf: &mut String, proof_depth: &mut usize) {
    if word_buf.is_empty() {
        return;
    }
    match word_buf.as_str() {
        "Proof" => *proof_depth += 1,
        "Qed" | "Defined" | "Admitted" | "Abort" => {
            if *proof_depth > 0 {
                *proof_depth -= 1;
            }
        }
        _ => {}
    }
    word_buf.clear();
}

fn flush_code(buf: &mut String, segments: &mut Vec<Segment>) {
    let taken = std::mem::take(buf);
    if !taken.trim().is_empty() {
        segments.push(Segment::Code(taken));
    }
}

fn flush_markdown(buf: &mut String, segments: &mut Vec<Segment>) {
    let taken = std::mem::take(buf);
    if !taken.trim().is_empty() {
        segments.push(Segment::Markdown(taken));
    }
}

/// Parses Rocq source text into a sequence of Markdown/Code segments.
///
/// # Arguments
///
/// * `content` - the full contents of a Rocq source file
///
/// # Returns
///
/// A tuple of the parsed `Segment`s and a `ConversionReport` describing any
/// warnings (for example, comment blocks that were not converted because
/// they did not start at the beginning of a line) and the number of
/// `--#--` blocks stripped from the output.
pub fn build_segments(content: &str) -> (Vec<Segment>, ConversionReport) {
    let chars: Vec<char> = content.chars().collect();
    let n = chars.len();

    let mut state = ParserState::new();
    let mut segments = Vec::new();
    let mut report = ConversionReport::default();
    let mut code_buf = String::new();
    let mut md_buf = String::new();
    let mut word_buf = String::new();

    let mut i = 0;
    while i < n {
        let c = chars[i];

        if state.depth == 0 {
            // Look for the start of a comment.
            if c == '(' && i + 1 < n && chars[i + 1] == '*' {
                check_word(&mut word_buf, &mut state.proof_depth);

                let at_col0 = i == 0 || chars[i - 1] == '\n';
                state.depth = 1;
                state.convert = at_col0 && state.proof_depth == 0;

                if !state.convert && state.proof_depth == 0 {
                    report
                        .warnings
                        .push(Warning::CommentNotAtLineStart { line: state.line_no });
                }

                if state.convert {
                    flush_code(&mut code_buf, &mut segments);
                } else {
                    code_buf.push('(');
                    code_buf.push('*');
                }

                i += 2;

                if state.convert {
                    // Skip blank spaces/tabs immediately following `(*` (but not newlines).
                    while i < n && (chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                }
                continue;
            }

            // Regular code character.
            if c.is_alphanumeric() || c == '_' {
                word_buf.push(c);
            } else {
                check_word(&mut word_buf, &mut state.proof_depth);
            }
            code_buf.push(c);
            if c == '\n' {
                state.line_no += 1;
            }
            i += 1;
            continue;
        }

        // state.depth >= 1: we are inside a (possibly nested) comment.
        if c == '(' && i + 1 < n && chars[i + 1] == '*' {
            state.depth += 1;
            if state.convert {
                md_buf.push('(');
                md_buf.push('*');
            } else {
                code_buf.push('(');
                code_buf.push('*');
            }
            i += 2;
            continue;
        }

        if c == '*' && i + 1 < n && chars[i + 1] == ')' {
            state.depth -= 1;
            if state.depth == 0 {
                if state.convert {
                    // Trim trailing blank spaces/tabs immediately before `*)`.
                    while md_buf.ends_with(' ') || md_buf.ends_with('\t') {
                        md_buf.pop();
                    }
                    flush_markdown(&mut md_buf, &mut segments);
                } else {
                    code_buf.push('*');
                    code_buf.push(')');
                }
            } else if state.convert {
                md_buf.push('*');
                md_buf.push(')');
            } else {
                code_buf.push('*');
                code_buf.push(')');
            }
            i += 2;
            continue;
        }

        // Regular character inside a comment. The `--#-- ... --#--` strip
        // marker is honored whether or not this comment is being converted
        // to Markdown: marked text (and the markers themselves) are always
        // removed, along with any redundant blank space left at the seam.
        if c == '-'
            && i + 4 < n
            && chars[i + 1] == '-'
            && chars[i + 2] == '#'
            && chars[i + 3] == '-'
            && chars[i + 4] == '-'
        {
            let buf = if state.convert { &mut md_buf } else { &mut code_buf };
            let was_open = state.strip_mode;

            if !was_open {
                // Opening marker: trim trailing blank spaces/tabs already
                // accumulated, remembering whether there were any so we can
                // decide whether to reinsert a single space once the
                // matching closing marker is found.
                let mut had_space = false;
                while buf.ends_with(' ') || buf.ends_with('\t') {
                    buf.pop();
                    had_space = true;
                }
                state.strip_had_space_before = had_space;
                state.strip_mode = true;
                i += 5;
                continue;
            } else {
                // Closing marker: also skip blank spaces/tabs immediately
                // following it, then reinsert a single space if there was
                // blank space on either side of the removed block.
                state.strip_mode = false;
                report.stripped_blocks += 1;
                i += 5;

                let mut had_space_after = false;
                while i < n && (chars[i] == ' ' || chars[i] == '\t') {
                    i += 1;
                    had_space_after = true;
                }
                if (state.strip_had_space_before || had_space_after) && !buf.is_empty() {
                    buf.push(' ');
                }
                continue;
            }
        }

        if !state.strip_mode {
            if state.convert {
                md_buf.push(c);
            } else {
                code_buf.push(c);
            }
        }
        if c == '\n' {
            state.line_no += 1;
        }
        i += 1;
    }

    check_word(&mut word_buf, &mut state.proof_depth);

    if state.depth != 0 {
        report.warnings.push(Warning::UnclosedComment);
        if state.convert {
            code_buf.push_str(&md_buf);
            md_buf.clear();
        }
    }

    flush_code(&mut code_buf, &mut segments);
    flush_markdown(&mut md_buf, &mut segments);

    (segments, report)
}

/// Merges parsed segments into the final Markdown document.
fn merge_segments(segments: &[Segment]) -> String {
    let mut result = String::new();

    for segment in segments {
        match segment {
            Segment::Code(content) => {
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    continue;
                }
                result.push_str("```rocq\n");
                result.push_str(trimmed);
                result.push_str("\n```\n\n");
            }
            Segment::Markdown(content) => {
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    continue;
                }
                result.push_str(trimmed);
                result.push_str("\n\n");
            }
        }
    }

    result.trim_end().to_string() + "\n"
}

/// Converts Rocq source text into a Markdown document.
///
/// Returns the generated Markdown together with a `ConversionReport`
/// describing any warnings (e.g. comment blocks that were skipped because
/// they did not start at the beginning of a line) and the number of
/// `--#--` blocks stripped from the output.
pub fn convert_to_markdown(content: &str) -> (String, ConversionReport) {
    let (segments, report) = build_segments(content);
    (merge_segments(&segments), report)
}

/// Name of the log file written to the current working directory whenever
/// a `rocq2md` run produces warnings.
const LOG_FILE_NAME: &str = "rocq2md.log";

/// Aggregates warnings and statistics for an entire `rocq2md` invocation,
/// which may involve converting a single file or an entire directory tree.
/// Used to produce the on-disk log and the stderr summary at the end of a
/// run.
#[derive(Default)]
struct RunStats {
    log_lines: Vec<String>,
    not_at_line_start: usize,
    unclosed_comment: usize,
    stripped_blocks: usize,
}

impl RunStats {
    fn record(&mut self, src_file: &Path, report: &ConversionReport) {
        for warning in &report.warnings {
            match warning {
                Warning::CommentNotAtLineStart { .. } => self.not_at_line_start += 1,
                Warning::UnclosedComment => self.unclosed_comment += 1,
            }
            self.log_lines
                .push(format!("{}: {}", src_file.display(), warning));
        }
        self.stripped_blocks += report.stripped_blocks;
    }

    fn warning_count(&self) -> usize {
        self.not_at_line_start + self.unclosed_comment
    }

    /// Writes the log file if there are warnings (removing any stale log
    /// left over from a previous, cleaner run), and prints a short summary
    /// to stderr.
    fn finish(&self) {
        let log_path = Path::new(LOG_FILE_NAME);

        if self.warning_count() == 0 {
            let _ = fs::remove_file(log_path);
            if self.stripped_blocks > 0 {
                eprintln!(
                    "Note: {} block(s) ignored due to --#-- markers.",
                    self.stripped_blocks
                );
            }
            return;
        }

        let mut log_content = self.log_lines.join("\n");
        log_content.push('\n');
        if let Err(e) = fs::write(log_path, log_content) {
            eprintln!("warning: failed to write {}: {}", log_path.display(), e);
        }

        eprintln!(
            "{} warning(s): {} unconverted comment(s) and {} unclosed comment(s); see {} for details.",
            self.warning_count(),
            self.not_at_line_start,
            self.unclosed_comment,
            log_path.display()
        );
        if self.stripped_blocks > 0 {
            eprintln!(
                "Note: {} block(s) ignored due to --#-- markers.",
                self.stripped_blocks
            );
        }
    }
}

/// Converts a single file and records any warnings into `stats`, without
/// writing the log file or printing the stderr summary (used internally so
/// that a directory conversion can aggregate results across many files
/// before finalizing).
fn convert_one_file(
    src_file: &Path,
    tgt_file: &Path,
    stats: &mut RunStats,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Converting {} to {}",
        src_file.display(),
        tgt_file.display()
    );

    let content = fs::read_to_string(src_file)?;

    if let Some(parent) = tgt_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let (markdown, report) = convert_to_markdown(&content);
    stats.record(src_file, &report);

    fs::write(tgt_file, markdown)?;

    Ok(())
}

/// Processes a single Rocq file and converts it to Markdown.
///
/// Any warnings are written to `rocq2md.log` in the current working
/// directory (which is removed if the run produces no warnings), and a
/// short summary is printed to stderr.
///
/// # Arguments
///
/// * `src_file` - Path to the source Rocq file
/// * `tgt_file` - Path to the target Markdown file
///
/// # Returns
///
/// Result containing `()` on success or an error message on failure
pub fn process_file(src_file: &Path, tgt_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut stats = RunStats::default();
    convert_one_file(src_file, tgt_file, &mut stats)?;
    stats.finish();
    Ok(())
}

/// Recursively converts a directory of Rocq files, without writing the log
/// file or printing the stderr summary (see `process_directory`).
fn process_directory_inner(
    src_dir: &Path,
    tgt_dir: &Path,
    stats: &mut RunStats,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(tgt_dir)?;

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let src_subdir = path.file_name().unwrap();
            let tgt_subdir = tgt_dir.join(src_subdir);
            process_directory_inner(&path, &tgt_subdir, stats)?;
        } else if let Some(ext) = path.extension() {
            if ext == "v" {
                let md_path: PathBuf = tgt_dir.join(path.file_stem().unwrap()).with_extension("md");
                convert_one_file(&path, &md_path, stats)?;
            }
        }
    }

    Ok(())
}

/// Processes a directory of Rocq files and converts them to Markdown.
///
/// Any warnings across all converted files are written to `rocq2md.log` in
/// the current working directory (which is removed if the run produces no
/// warnings), and a short summary is printed to stderr.
///
/// # Arguments
///
/// * `src_dir` - Path to the source directory containing Rocq files
/// * `tgt_dir` - Path to the target directory where Markdown files will be created
///
/// # Returns
///
/// Result containing `()` on success or an error message on failure
pub fn process_directory(src_dir: &Path, tgt_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut stats = RunStats::default();
    process_directory_inner(src_dir, tgt_dir, &mut stats)?;
    stats.finish();
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_level_comment_converted() {
        let input = "(* This is documentation. *)\nDefinition foo := 1.";
        let (segments, report) = build_segments(input);
        assert!(report.warnings.is_empty());
        assert_eq!(segments.len(), 2);
        assert_eq!(
            segments[0],
            Segment::Markdown("This is documentation.".to_string())
        );
        match &segments[1] {
            Segment::Code(c) => assert!(c.contains("Definition foo := 1.")),
            _ => panic!("expected code segment"),
        }
    }

    #[test]
    fn test_multiline_comment_converted() {
        let input = "(*\nHello world.\nMore text.\n*)\nDefinition foo := 1.";
        let (segments, _) = build_segments(input);
        match &segments[0] {
            Segment::Markdown(c) => {
                assert_eq!(c, "\nHello world.\nMore text.\n");
            }
            _ => panic!("expected markdown segment"),
        }
    }

    #[test]
    fn test_comment_not_at_column_zero_kept_as_code_with_warning() {
        let input = "Definition foo := 1. (* trailing note *)\nDefinition bar := 2.";
        let (segments, report) = build_segments(input);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            Segment::Code(c) => assert!(c.contains("(* trailing note *)")),
            _ => panic!("expected code segment"),
        }
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0], Warning::CommentNotAtLineStart { line: 1 });
    }

    #[test]
    fn test_comment_inside_proof_no_warning() {
        let input = "Lemma foo : True.\nProof.\n(* This explains the tactic. *)\ntrivial.\nQed.";
        let (segments, report) = build_segments(input);
        assert!(report.warnings.is_empty());
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            Segment::Code(c) => assert!(c.contains("(* This explains the tactic. *)")),
            _ => panic!("expected code segment"),
        }
    }

    #[test]
    fn test_nested_comment_preserved_inside_converted_block() {
        let input = "(* outer (* nested *) still outer *)\nDefinition foo := 1.";
        let (segments, report) = build_segments(input);
        assert!(report.warnings.is_empty());
        match &segments[0] {
            Segment::Markdown(c) => assert_eq!(c, "outer (* nested *) still outer"),
            _ => panic!("expected markdown segment"),
        }
    }

    #[test]
    fn test_strip_marker_inline() {
        let input = "(* Keep this --#-- remove this --#-- keep this too *)\nDefinition foo := 1.";
        let (segments, report) = build_segments(input);
        match &segments[0] {
            Segment::Markdown(c) => assert_eq!(c, "Keep this keep this too"),
            _ => panic!("expected markdown segment"),
        }
        assert_eq!(report.stripped_blocks, 1);
    }

    #[test]
    fn test_strip_marker_block() {
        let input =
            "(*\nKeep this line.\n--#--\nThis should be removed.\nAnd this too.\n--#--\nKeep this too.\n*)\nDefinition foo := 1.";
        let (segments, report) = build_segments(input);
        match &segments[0] {
            Segment::Markdown(c) => {
                assert!(c.contains("Keep this line."));
                assert!(c.contains("Keep this too."));
                assert!(!c.contains("This should be removed."));
                assert!(!c.contains("And this too."));
            }
            _ => panic!("expected markdown segment"),
        }
        assert_eq!(report.stripped_blocks, 1);
    }

    #[test]
    fn test_strip_marker_in_comment_not_at_column_zero() {
        // The comment is kept as code (it does not start at column 0), but
        // the --#-- marked text should still be removed, along with the
        // markers themselves and the redundant blank space at the seam.
        let input = " (* Keep this --#-- remove this --#-- keep this too *)\nDefinition foo := 1.";
        let (segments, report) = build_segments(input);
        assert_eq!(report.stripped_blocks, 1);
        match &segments[0] {
            Segment::Code(c) => {
                assert!(c.contains("(* Keep this keep this too *)"));
                assert!(!c.contains("remove this"));
                assert!(!c.contains("--#--"));
            }
            _ => panic!("expected code segment"),
        }
    }

    #[test]
    fn test_strip_marker_in_comment_inside_proof() {
        // The comment is kept as code because it is inside a proof, but the
        // --#-- marked text should still be removed.
        let input = "Lemma foo : True.\nProof.\n(* Keep this --#-- remove this --#-- keep this too *)\ntrivial.\nQed.";
        let (segments, report) = build_segments(input);
        assert!(report.warnings.is_empty());
        assert_eq!(report.stripped_blocks, 1);
        match &segments[0] {
            Segment::Code(c) => {
                assert!(c.contains("(* Keep this keep this too *)"));
                assert!(!c.contains("remove this"));
                assert!(!c.contains("--#--"));
            }
            _ => panic!("expected code segment"),
        }
    }

    #[test]
    fn test_unclosed_comment_at_eof_warns() {
        let input = "(* This comment never closes.\nDefinition foo := 1.";
        let (segments, report) = build_segments(input);

        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0], Warning::UnclosedComment);

        // Since the comment was never closed, its content is folded back
        // into a code segment rather than being emitted as Markdown.
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            Segment::Code(c) => {
                assert!(c.contains("This comment never closes."));
                assert!(c.contains("Definition foo := 1."));
            }
            _ => panic!("expected code segment"),
        }
    }

    #[test]
    fn test_merge_segments_produces_code_fences() {
        let input = "(* Intro text *)\nDefinition foo := 1.";
        let (markdown, _) = convert_to_markdown(input);
        assert!(markdown.contains("Intro text"));
        assert!(markdown.contains("```rocq"));
        assert!(markdown.contains("Definition foo := 1."));
        assert!(markdown.contains("```\n"));
    }
}
