use std::fs;
use std::path::{Path, PathBuf};

/// Either Markdown text or a chunk of Rocq source, in output order.
#[derive(Debug, PartialEq)]
pub enum Segment {
    Markdown(String),
    Code(String),
}

/// A parsing anomaly found in a Rocq source file.
#[derive(Debug, PartialEq)]
pub enum Warning {
    /// Comment kept as code because `(*` wasn't at the start of a line.
    CommentNotAtLineStart { line: usize },
    /// Comment still open at end of file.
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

/// Warnings and stripped-block count from converting one file.
#[derive(Debug, Default, PartialEq)]
pub struct ConversionReport {
    pub warnings: Vec<Warning>,
    pub stripped_blocks: usize,
}

/// State needed to decide whether a `(* ... *)` comment becomes Markdown.
struct ParserState {
    depth: usize,                 // (* *) nesting depth; 0 = in code
    convert: bool,                // is the open top-level comment -> Markdown?
    proof_depth: usize,           // nesting depth of Proof...Qed/Defined/Admitted/Abort
    strip_mode: bool,             // are we inside an open --#-- marker?
    strip_had_space_before: bool, // was there blank space before that marker?
    line_no: usize,               // line number to index warning messages
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

/// Updates the proof-nesting counter based on a completed identifier.
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

/// Parses Rocq source into Markdown/Code segments, plus a warning report.
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
            // Comment start.
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
                    // Skip blank spaces/tabs right after `(*` (not newlines).
                    while i < n && (chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                }
                continue;
            }
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

        // Inside a (possibly nested) comment.
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
                    // Trim trailing blank spaces/tabs right before `*)`.
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

        // --#-- strip marker: removes marked text and collapses seam whitespace to one space.
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

/// Joins segments into the final Markdown document.
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

/// Converts Rocq source text into a Markdown document plus a warning report.
pub fn convert_to_markdown(content: &str) -> (String, ConversionReport) {
    let (segments, report) = build_segments(content);
    (merge_segments(&segments), report)
}

/// Log file written to the current directory when a run produces warnings.
const LOG_FILE_NAME: &str = "rocq2md.log";

/// Aggregated warnings/stats across a run (single file or directory tree).
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

    /// Writes the log (or clears a stale one) and prints a stderr summary.
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

/// Converts one file and records warnings into `stats` (no log/stderr output).
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

/// Converts a single Rocq file to Markdown, writing `rocq2md.log` on warnings.
pub fn process_file(src_file: &Path, tgt_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut stats = RunStats::default();
    convert_one_file(src_file, tgt_file, &mut stats)?;
    stats.finish();
    Ok(())
}

/// Recursively converts a directory tree (no log/stderr output here).
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

/// Converts a directory of `.v` files to Markdown, writing `rocq2md.log` on warnings.
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
        // Kept as code (not at column 0), but --#-- text is still stripped.
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
        // Kept as code (inside a proof), but --#-- text is still stripped.
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
        // Unclosed comment content folds back into code, not Markdown.
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
