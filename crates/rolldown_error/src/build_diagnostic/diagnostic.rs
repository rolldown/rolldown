use std::{fmt::Display, fmt::Write as _, ops::Range};

use arcstr::ArcStr;
use ariadne::{Config, Label, Report, ReportBuilder, ReportKind, Span, sources};
use rustc_hash::FxHashMap;

use crate::utils::{ByteLocator, is_context_too_long};

use super::Severity;

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq)]
pub struct DiagnosticFileId(ArcStr);

impl Display for DiagnosticFileId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}

impl From<ArcStr> for DiagnosticFileId {
  fn from(value: ArcStr) -> Self {
    Self(value)
  }
}

#[derive(Debug, Clone)]
pub struct RolldownLabelSpan(DiagnosticFileId, Range<usize>);

impl ariadne::Span for RolldownLabelSpan {
  type SourceId = DiagnosticFileId;

  fn source(&self) -> &Self::SourceId {
    &self.0
  }

  fn start(&self) -> usize {
    self.1.start
  }

  fn end(&self) -> usize {
    self.1.end
  }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
  pub(crate) kind: String,
  pub(crate) title: String,
  pub(crate) files: FxHashMap</* filename */ DiagnosticFileId, /* file content */ ArcStr>,
  pub(crate) labels: Vec<Label<RolldownLabelSpan>>,
  pub(crate) helps: Vec<String>,
  pub(crate) note: Option<String>,
  pub(crate) severity: Severity,
}

type AriadneReportBuilder = ReportBuilder<'static, RolldownLabelSpan>;
type AriadneReport = Report<'static, RolldownLabelSpan>;

impl Diagnostic {
  pub(crate) fn new(kind: String, summary: String, severity: Severity) -> Self {
    Self {
      kind,
      title: summary,
      files: FxHashMap::default(),
      labels: Vec::default(),
      helps: Vec::default(),
      note: None,
      severity,
    }
  }

  pub(crate) fn add_file(
    &mut self,
    filename: impl Into<ArcStr>,
    content: impl Into<ArcStr>,
  ) -> DiagnosticFileId {
    let filename = DiagnosticFileId::from(filename.into());
    let content = content.into();
    debug_assert!(!self.files.contains_key(&filename));
    self.files.insert(filename.clone(), content);
    filename
  }

  #[inline]
  pub(crate) fn add_help(&mut self, message: String) -> &mut Self {
    self.helps.push(message);
    self
  }

  #[inline]
  pub(crate) fn add_note(&mut self, message: String) -> &mut Self {
    self.note = Some(message);
    self
  }

  pub(crate) fn add_label(
    &mut self,
    file_id: &DiagnosticFileId,
    range: impl Into<Range<u32>>,
    message: String,
  ) -> &mut Self {
    let range = range.into();
    let range = range.start as usize..range.end as usize;
    let label =
      Label::new(RolldownLabelSpan(file_id.0.clone().into(), range)).with_message(message);
    self.labels.push(label);
    self
  }

  fn init_report_builder(&self) -> AriadneReportBuilder {
    let mut message = self.title.clone();
    let mut builder = AriadneReport::build(
      match self.severity {
        Severity::Info => ReportKind::Advice,
        Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
      },
      RolldownLabelSpan(ArcStr::default().into(), 0..0),
    )
    .with_code(self.kind.clone());

    for label in self.labels.clone() {
      if is_context_too_long(&label, &self.files) {
        let span = label.span();
        let _ = write!(
          message,
          "\n - {} in {} at {:?}",
          label.display_info().msg().unwrap_or_default(),
          span.source(),
          span.start()..span.end()
        );
      } else {
        builder = builder.with_label(label);
      }
    }

    builder.with_helps(self.helps.clone());

    if let Some(note) = &self.note {
      builder = builder.with_note(note);
    }

    builder = builder.with_message(message);

    builder
  }

  pub fn convert_to_string(&self, color: bool) -> String {
    let mut cache = sources(self.files.clone());
    self.convert_to_string_with_cache(color, &mut cache)
  }

  /// Like [`Self::convert_to_string`], but renders against a caller-owned ariadne
  /// source cache so the line-indexed `Source` for each file is built once and
  /// reused across many diagnostics instead of rebuilt per render (see #9748).
  fn convert_to_string_with_cache(
    &self,
    color: bool,
    cache: &mut impl ariadne::Cache<DiagnosticFileId>,
  ) -> String {
    let builder = self.init_report_builder();
    let mut output = Vec::new();
    let result = builder
      .with_config(
        Config::default()
          .with_color(color)
          .with_index_type(ariadne::IndexType::Byte)
          .with_severity_prefix(false),
      )
      .finish()
      .write_for_stdout(&mut *cache, &mut output);
    match result {
      Ok(()) => String::from_utf8_lossy(&output).into_owned(),
      Err(_) => format!("[{}] {}", self.kind, self.title),
    }
  }

  pub fn to_color_string(&self) -> String {
    self.convert_to_string(true)
  }

  /// Get the primary location information from the first label if available
  /// Returns (file_path, line, column, utf16_position)
  pub fn get_primary_location(&self) -> Option<(String, usize, usize, usize)> {
    let first_label = self.labels.first()?;
    let span = first_label.span();
    let start = span.start();
    let source = self.files.get(span.source())?;

    let mut line = 1; // 1-based
    let mut column = 0; // 0-based
    let mut utf16_pos = 0;
    let mut byte_count = 0;

    for ch in source.chars() {
      if byte_count >= start {
        break;
      }
      if ch == '\n' {
        line += 1;
        column = 0;
      } else {
        column += ch.len_utf16();
      }
      utf16_pos += ch.len_utf16();
      byte_count += ch.len_utf8();
    }

    let file = span.source().to_string();
    Some((file, line, column, utf16_pos))
  }

  pub fn kind(&self) -> String {
    self.kind.clone()
  }

  /// Code frame around the error's first label.
  pub fn frame(&self) -> Option<String> {
    let span = self.labels.first()?.span();
    let source = self.files.get(span.source())?;
    let frame = render_code_frame(source, span.start());
    (!frame.is_empty()).then_some(frame)
  }

  /// Render many diagnostics at once, sharing per-source work across all of them.
  ///
  /// Rendering diagnostics one by one rebuilds the ariadne `Source` (its rope and
  /// full line index) for the whole file on every diagnostic, so emitting N
  /// diagnostics that point into the same large file is O(N^2) and can appear to
  /// hang the build (#9748). Sharing the `Source` cache and a per-source line table
  /// removes that quadratic factor — the line containing an offset is found in
  /// O(log lines) per diagnostic, then its UTF-16 column is read by scanning only
  /// that one line (see [`ByteLocator::locate_utf16`]).
  ///
  /// One residual cost is outside this batching: ariadne prints the entire labelled
  /// line for every diagnostic, so a pathological input where N diagnostics all land
  /// on one very long line (e.g. a minified bundle) still costs O(N * line_len)
  /// inside ariadne itself — the within-line column scan is bounded by that same
  /// line and is not the deciding factor. The cross-diagnostic O(N^2) blow-up that
  /// actually caused the reported hang is what this removes.
  pub fn render_batch(diagnostics: &[Diagnostic], color: bool) -> Vec<RenderedDiagnostic> {
    // Collect every source referenced by any diagnostic, deduplicated, so the
    // ariadne cache builds each `Source` exactly once.
    let mut files: FxHashMap<DiagnosticFileId, ArcStr> = FxHashMap::default();
    for diagnostic in diagnostics {
      for (id, content) in &diagnostic.files {
        files.entry(id.clone()).or_insert_with(|| content.clone());
      }
    }
    let mut cache = sources(files.iter().map(|(id, content)| (id.clone(), content.clone())));
    let mut locators: FxHashMap<DiagnosticFileId, ByteLocator> = FxHashMap::default();

    diagnostics
      .iter()
      .map(|diagnostic| {
        let primary_location = diagnostic.primary_location_with(&files, &mut locators);
        let message = diagnostic.convert_to_string_with_cache(color, &mut cache);
        RenderedDiagnostic { message, primary_location }
      })
      .collect()
  }

  /// Compute the primary location using a caller-owned per-source line-table
  /// cache, so the line index for each file is built once across many lookups.
  fn primary_location_with(
    &self,
    files: &FxHashMap<DiagnosticFileId, ArcStr>,
    locators: &mut FxHashMap<DiagnosticFileId, ByteLocator>,
  ) -> Option<DiagnosticPrimaryLocation> {
    let first_label = self.labels.first()?;
    let span = first_label.span();
    let source_id = span.source();
    let source = files.get(source_id)?;
    let locator = locators.entry(source_id.clone()).or_insert_with(|| ByteLocator::new(source));
    let (line, column, utf16_position) = locator.locate_utf16(source, span.start());
    Some(DiagnosticPrimaryLocation { line, column, utf16_position })
  }
}

const LINE_TRUNCATE_LENGTH: usize = 120;
const MIN_CHARACTERS_SHOWN_AFTER_LOCATION: usize = 10;
const ELLIPSIS: &str = "...";

// Only leading tabs are expanded, each to two spaces.
fn tabs_to_spaces(value: &str) -> String {
  let leading_tabs = value.bytes().take_while(|&byte| byte == b'\t').count();
  if leading_tabs == 0 {
    return value.to_string();
  }
  let mut expanded = "  ".repeat(leading_tabs);
  expanded.push_str(&value[leading_tabs..]);
  expanded
}

fn render_code_frame(source: &str, byte_offset: usize) -> String {
  let offset = byte_offset.min(source.len());
  let before = source.get(..offset).unwrap_or(source);
  let line = before.bytes().filter(|&byte| byte == b'\n').count() + 1;
  let byte_column = offset - before.rfind('\n').map_or(0, |index| index + 1);

  let all: Vec<&str> = source.split('\n').collect();
  if line > all.len() {
    return String::new();
  }
  let error_line = all[line - 1];
  let location_width =
    tabs_to_spaces(error_line.get(..byte_column).unwrap_or(error_line)).chars().count();
  let max_line_length = (location_width + MIN_CHARACTERS_SHOWN_AFTER_LOCATION + ELLIPSIS.len())
    .max(LINE_TRUNCATE_LENGTH);

  let frame_start = line.saturating_sub(3);
  let mut frame_end = (line + 2).min(all.len());
  let mut lines: Vec<&str> = all[frame_start..frame_end].to_vec();
  while lines.last().is_some_and(|last| last.trim().is_empty()) {
    lines.pop();
    frame_end -= 1;
  }

  let digits = frame_end.to_string().len();
  lines
    .iter()
    .enumerate()
    .map(|(index, source_line)| {
      let line_no = frame_start + index + 1;
      let mut displayed = tabs_to_spaces(source_line);
      if displayed.chars().count() > max_line_length {
        displayed = displayed.chars().take(max_line_length - ELLIPSIS.len()).collect();
        displayed.push_str(ELLIPSIS);
      }
      if line_no == line {
        let indent = digits + 2 + location_width;
        format!("{line_no:>digits$}: {displayed}\n{:indent$}^", "")
      } else {
        format!("{line_no:>digits$}: {displayed}")
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Primary source location of a diagnostic's first label.
#[derive(Debug, Clone)]
pub struct DiagnosticPrimaryLocation {
  /// 1-based line number.
  pub line: usize,
  /// 0-based UTF-16 column within the line.
  pub column: usize,
  /// UTF-16 code-unit offset from the start of the file.
  pub utf16_position: usize,
}

/// A diagnostic rendered to its display string together with its primary location.
#[derive(Debug, Clone)]
pub struct RenderedDiagnostic {
  pub message: String,
  pub primary_location: Option<DiagnosticPrimaryLocation>,
}

impl Display for Diagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.convert_to_string(false).fmt(f)
  }
}

#[cfg(test)]
mod tests {
  use super::{Severity, render_code_frame};

  #[test]
  fn code_frame_points_at_offset() {
    assert_eq!(render_code_frame("a\nbcd\nef", 3), "1: a\n2: bcd\n    ^\n3: ef");
  }

  #[test]
  fn code_frame_expands_tabs_and_aligns_caret() {
    assert_eq!(render_code_frame("\tfoo", 1), "1:   foo\n     ^");
  }

  #[test]
  fn code_frame_preserves_non_leading_tabs() {
    assert_eq!(render_code_frame("ab\tcd", 3), "1: ab\tcd\n      ^");
  }

  #[test]
  fn code_frame_truncates_long_lines() {
    let frame = render_code_frame(&"x".repeat(200), 0);
    let first_line = frame.lines().next().unwrap();
    assert_eq!(first_line, format!("1: {}...", "x".repeat(117)));
  }

  #[test]
  fn frame_returns_none_without_labels() {
    let diagnostic = super::Diagnostic::new("X".to_string(), "title".to_string(), Severity::Error);
    assert_eq!(diagnostic.frame(), None);
  }
}
