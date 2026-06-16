use std::{fmt::Display, fmt::Write as _, ops::Range};

use arcstr::ArcStr;
use ariadne::{Config, Label, Report, ReportBuilder, ReportKind, Span, sources};
use rustc_hash::FxHashMap;

use crate::utils::is_context_too_long;

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

impl From<(DiagnosticFileId, Range<usize>)> for RolldownLabelSpan {
  fn from((id, range): (DiagnosticFileId, Range<usize>)) -> Self {
    Self(id, range)
  }
}

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
        write!(
          message,
          "\n - {} in {} at {:?}",
          label.display_info().msg().unwrap_or_default(),
          span.source(),
          span.start()..span.end()
        )
        .ok();
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

  #[must_use]
  pub fn with_kind(mut self, kind: String) -> Self {
    self.kind = kind;
    self
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

  /// Render many diagnostics at once, sharing per-source work across all of them.
  ///
  /// Rendering diagnostics one by one rebuilds the ariadne `Source` (its rope and
  /// full line index) for the whole file on every diagnostic, so emitting N
  /// diagnostics that point into the same large file is O(N^2) and can appear to
  /// hang the build (#9748). Sharing the `Source` cache and a per-source line table
  /// removes that quadratic factor — the line containing an offset is found in
  /// O(log lines) per diagnostic, then its UTF-16 column is read by scanning only
  /// that one line (see [`LineTable`]).
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
    let mut line_tables: FxHashMap<DiagnosticFileId, LineTable> = FxHashMap::default();

    diagnostics
      .iter()
      .map(|diagnostic| {
        let primary_location = diagnostic.primary_location_with(&files, &mut line_tables);
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
    line_tables: &mut FxHashMap<DiagnosticFileId, LineTable>,
  ) -> Option<DiagnosticPrimaryLocation> {
    let first_label = self.labels.first()?;
    let span = first_label.span();
    let source_id = span.source();
    let source = files.get(source_id)?;
    let table = line_tables.entry(source_id.clone()).or_insert_with(|| LineTable::new(source));
    let (line, column, utf16_position) = table.locate(source, span.start());
    Some(DiagnosticPrimaryLocation { line, column, utf16_position })
  }
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

/// Precomputed line-start offsets for one source. Finding the line that contains a
/// byte offset is O(log lines) instead of scanning from the start of the file; the
/// UTF-16 column within that line is then read by scanning the line's prefix (see
/// [`Self::locate`]).
struct LineTable {
  /// Byte offset of the start of each line.
  line_byte_starts: Vec<usize>,
  /// UTF-16 offset (from file start) of the start of each line, parallel to
  /// `line_byte_starts`.
  line_utf16_starts: Vec<usize>,
}

impl LineTable {
  fn new(source: &str) -> Self {
    let mut line_byte_starts = vec![0usize];
    let mut line_utf16_starts = vec![0usize];
    let mut utf16 = 0usize;
    for (byte_idx, ch) in source.char_indices() {
      utf16 += ch.len_utf16();
      if ch == '\n' {
        line_byte_starts.push(byte_idx + ch.len_utf8());
        line_utf16_starts.push(utf16);
      }
    }
    Self { line_byte_starts, line_utf16_starts }
  }

  /// Returns `(1-based line, 0-based utf16 column, utf16 offset from file start)`
  /// for `byte_offset`.
  fn locate(&self, source: &str, byte_offset: usize) -> (usize, usize, usize) {
    // Index of the last line whose start is <= byte_offset. `line_byte_starts`
    // always begins with 0, so `partition_point` is >= 1 and the `- 1` is safe.
    let line_idx = self.line_byte_starts.partition_point(|&start| start <= byte_offset) - 1;
    let line_byte_start = self.line_byte_starts[line_idx];
    let line_utf16_start = self.line_utf16_starts[line_idx];
    // Clamp to the source length so a (malformed) out-of-range offset degrades to
    // the end-of-file position instead of yielding column 0.
    let end = byte_offset.min(source.len());
    // UTF-16 column within the line. Linear in the line's length, which is short in
    // practice; the pathological single-very-long-line case is bounded by ariadne
    // re-rendering that whole line per diagnostic anyway (see `render_batch`).
    let column: usize = source
      .get(line_byte_start..end)
      .map(|within_line| within_line.chars().map(char::len_utf16).sum())
      .unwrap_or(0);
    (line_idx + 1, column, line_utf16_start + column)
  }
}

impl Display for Diagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.convert_to_string(false).fmt(f)
  }
}

#[cfg(test)]
mod tests {
  use super::LineTable;

  /// Straightforward, independent reference for [`LineTable::locate`] used to
  /// validate it. `byte_offset` must fall on a char boundary.
  fn reference_locate(source: &str, byte_offset: usize) -> (usize, usize, usize) {
    let end = byte_offset.min(source.len());
    let prefix = &source[..end];
    let utf16_position: usize = prefix.chars().map(char::len_utf16).sum();
    let line0 = prefix.matches('\n').count();
    let line_byte_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let column: usize = source[line_byte_start..end].chars().map(char::len_utf16).sum();
    (line0 + 1, column, utf16_position)
  }

  fn assert_matches_reference(source: &str) {
    let table = LineTable::new(source);
    for byte_offset in 0..=source.len() {
      if !source.is_char_boundary(byte_offset) {
        continue;
      }
      assert_eq!(
        table.locate(source, byte_offset),
        reference_locate(source, byte_offset),
        "mismatch at byte offset {byte_offset} of {source:?}"
      );
    }
    // An out-of-range offset clamps to the end-of-file position.
    let past_end = source.len() + 5;
    assert_eq!(
      table.locate(source, past_end),
      reference_locate(source, past_end),
      "mismatch at out-of-range offset {past_end} of {source:?}"
    );
  }

  #[test]
  fn line_table_matches_linear_reference() {
    for source in [
      "",              // empty source / single-entry table + out-of-range clamp
      "a\nbc\n\ndef",  // multiple lines incl. an empty one: binary-search line lookup
      "café\n😀xy\nz", // 2-byte accent + astral emoji across lines: cumulative utf16 table
    ] {
      assert_matches_reference(source);
    }
  }
}
