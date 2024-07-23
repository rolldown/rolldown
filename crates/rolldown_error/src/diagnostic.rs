use crate::build_error::severity::Severity;
use arcstr::ArcStr;
use ariadne::{sources, Config, Label, Report, ReportBuilder, ReportKind};
use std::{fmt::Display, ops::Range};

#[derive(Debug, Clone)]
pub struct DiagnosticFileId(ArcStr);

#[derive(Debug, Clone)]
pub struct Diagnostic {
  pub(crate) kind: String,
  pub(crate) title: String,
  pub(crate) files: Vec<(/* filename */ ArcStr, /* file content */ ArcStr)>,
  pub(crate) labels: Vec<Label<(/* filename */ ArcStr, Range<usize>)>>,
  pub(crate) help: Option<String>,
  pub(crate) severity: Severity,
}

type AriadneReportBuilder = ReportBuilder<'static, (ArcStr, Range<usize>)>;
type AriadneReport = Report<'static, (ArcStr, Range<usize>)>;

impl Diagnostic {
  pub(crate) fn new(kind: String, summary: String, severity: Severity) -> Self {
    Self {
      kind,
      title: summary,
      files: Vec::default(),
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
    let filename = filename.into();
    let content = content.into();
    debug_assert!(self.files.iter().all(|(id, _)| id != &filename));
    self.files.push((filename.clone(), content));
    DiagnosticFileId(filename)
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
    let label = Label::new((file_id.0.clone(), range)).with_message(message);
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
    .with_code(self.kind.clone())
    .with_message(self.title.clone());

    for label in self.labels.clone() {
      builder = builder.with_label(label);
    }

    if let Some(help) = &self.help {
      builder = builder.with_help(help);
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
