use std::{fmt::Display, fmt::Write as _, ops::Range};

use arcstr::ArcStr;
use ariadne::{Config, Label, Report, ReportBuilder, ReportKind, Span, sources};
use rustc_hash::FxHashMap;

use crate::{build_error::severity::Severity, utils::is_context_too_long};

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
  pub(crate) help: Option<String>,
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
      help: None,
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

  pub(crate) fn add_help(&mut self, message: String) -> &mut Self {
    self.help = Some(message);
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
        .expect("Failed to write report message");
      } else {
        builder = builder.with_label(label);
      }
    }

    if let Some(help) = &self.help {
      builder = builder.with_help(help);
    }

    builder = builder.with_message(message);

    builder
  }

  pub fn convert_to_string(&self, color: bool) -> String {
    let builder = self.init_report_builder();
    let mut output = Vec::new();
    builder
      .with_config(Config::default().with_color(color).with_index_type(ariadne::IndexType::Byte))
      .finish()
      .write_for_stdout(sources(self.files.clone()), &mut output)
      .unwrap();
    String::from_utf8(output).expect("Diagnostic should be valid utf8")
  }

  pub fn to_color_string(&self) -> String {
    self.convert_to_string(true)
  }

  pub fn with_kind(mut self, kind: String) -> Self {
    self.kind = kind;
    self
  }
}

impl Display for Diagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.convert_to_string(false).fmt(f)
  }
}
