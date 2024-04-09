use crate::build_error::severity::Severity;
use ariadne::{sources, Config, Label, Report, ReportBuilder, ReportKind};
use std::{fmt::Display, ops::Range, sync::Arc};

#[derive(Debug, Clone)]
pub struct DiagnosticFileId(Arc<str>);

#[derive(Debug, Clone)]
pub struct Diagnostic {
  pub(crate) kind: &'static str,
  pub(crate) title: String,
  pub(crate) files: Vec<(/* filename */ Arc<str>, /* file content */ Arc<str>)>,
  pub(crate) labels: Vec<Label<(/* filename */ Arc<str>, Range<usize>)>>,
  pub(crate) severity: Severity,
}

type AriadneReportBuilder = ReportBuilder<'static, (Arc<str>, Range<usize>)>;
type AriadneReport = Report<'static, (Arc<str>, Range<usize>)>;

impl Diagnostic {
  pub(crate) fn new(kind: &'static str, summary: String, severity: Severity) -> Self {
    Self { kind, title: summary, files: Vec::default(), labels: Vec::default(), severity }
  }

  pub(crate) fn add_file(
    &mut self,
    filename: impl Into<Arc<str>>,
    content: impl Into<Arc<str>>,
  ) -> DiagnosticFileId {
    let filename = filename.into();
    let content = content.into();
    debug_assert!(self.files.iter().all(|(id, _)| id != &filename));
    self.files.push((Arc::clone(&filename), content));
    DiagnosticFileId(filename)
  }

  pub(crate) fn add_label(
    &mut self,
    file_id: &DiagnosticFileId,
    range: impl Into<Range<u32>>,
    message: String,
  ) -> &mut Self {
    let range = range.into();
    let range = range.start as usize..range.end as usize;
    let label = Label::new((Arc::clone(&file_id.0), range)).with_message(message);
    self.labels.push(label);
    self
  }

  fn init_report_builder(&mut self) -> AriadneReportBuilder {
    let mut builder = AriadneReport::build(
      match self.severity {
        Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
      },
      "",
      0,
    )
    .with_code(self.kind)
    .with_message(self.title.clone());

    for label in self.labels.clone() {
      builder = builder.with_label(label);
    }

    builder
  }

  pub fn convert_to_string(&self, color: bool) -> String {
    let builder = self.clone().init_report_builder();
    let mut output = Vec::new();
    builder
      .with_config(Config::default().with_color(color))
      .finish()
      .write_for_stdout(sources(self.files.clone()), &mut output)
      .unwrap();
    String::from_utf8(output).expect("Diagnostic should be valid utf8")
  }

  pub fn to_color_string(&self) -> String {
    self.convert_to_string(true)
  }
}

impl Display for Diagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.convert_to_string(false).fmt(f)
  }
}
