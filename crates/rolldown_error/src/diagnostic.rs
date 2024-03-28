use std::{fmt::Display, ops::Range};

use ariadne::{sources, Config, Label, Report, ReportBuilder, ReportKind};

use crate::error::Severity;

type Labels = Vec<Label<(String, Range<usize>)>>;

#[derive(Debug, Default)]
pub struct DiagnosticBuilder {
  pub code: Option<&'static str>,
  pub summary: Option<String>,
  pub files: Option<Vec<(String, String)>>,
  pub labels: Option<Labels>,
  pub severity: Option<Severity>,
}

impl DiagnosticBuilder {
  pub fn build(self) -> Diagnostic {
    Diagnostic {
      code: self.code.expect("Field `code` should be sett"),
      summary: self.summary.expect("Field `summary` should be set"),
      severity: self.severity.expect("Field `severity` should be set"),
      labels: self.labels.unwrap_or_default(),
      files: self.files.unwrap_or_default(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
  pub(crate) code: &'static str,
  pub(crate) summary: String,
  pub(crate) files: Vec<(String, String)>,
  pub(crate) labels: Labels,
  pub(crate) severity: Severity,
}

impl Diagnostic {
  fn init_report_builder(&mut self) -> ReportBuilder<'static, (String, Range<usize>)> {
    let mut builder = Report::<(String, Range<usize>)>::build(
      match self.severity {
        Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
      },
      "",
      0,
    )
    .with_code(self.code)
    .with_message(self.summary.clone());

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
