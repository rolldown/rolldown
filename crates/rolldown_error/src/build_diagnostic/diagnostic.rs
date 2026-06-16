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
    let mut start = range.start as usize;
    let mut end = range.end as usize;
    // `convert_to_string` renders with `IndexType::Byte`, so ariadne slices the source by
    // raw byte offsets and panics if a label boundary falls inside a multibyte UTF-8
    // character. Clamp to the source length and snap to char boundaries so a malformed
    // span degrades gracefully instead of crashing diagnostic rendering.
    if let Some(source) = self.files.get(file_id) {
      let len = source.len();
      start = start.min(len);
      end = end.clamp(start, len);
      while start > 0 && !source.is_char_boundary(start) {
        start -= 1;
      }
      while end < len && !source.is_char_boundary(end) {
        end += 1;
      }
    }
    let label =
      Label::new(RolldownLabelSpan(file_id.0.clone().into(), start..end)).with_message(message);
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
      .write_for_stdout(sources(self.files.clone()), &mut output);
    match result {
      Ok(()) => String::from_utf8_lossy(&output).into_owned(),
      Err(_) => format!("[{}] {}", self.kind, self.title),
    }
  }

  pub fn to_color_string(&self) -> String {
    self.convert_to_string(true)
  }

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
}

impl Display for Diagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.convert_to_string(false).fmt(f)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Regression test: a label span whose boundary falls inside a multibyte UTF-8
  /// character (or runs past EOF) must not panic when the diagnostic is rendered.
  /// `convert_to_string` uses `IndexType::Byte`, so rolldown-ariadne slices the
  /// source by raw byte offsets and previously panicked on such spans.
  #[test]
  fn label_span_inside_multibyte_char_does_not_panic() {
    // `é` occupies bytes 3..5; byte 4 is a UTF-8 continuation byte.
    let src = "{ \"é\": }";
    for range in [4u32..4, 3u32..4, 4u32..5, 0u32..200] {
      let mut d = Diagnostic::new("X".to_string(), "t".to_string(), Severity::Error);
      let f = d.add_file("a.json", src);
      d.add_label(&f, range.clone(), "here".to_string());
      let out = d.convert_to_string(false);
      assert!(!out.is_empty(), "rendering produced no output for span {range:?}");
    }
  }
}
