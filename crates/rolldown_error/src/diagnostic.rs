use ariadne::{sources, Config, Label, Report, ReportBuilder, ReportKind};
use std::{fmt::Display, ops::Range};

#[derive(Debug, Default, Clone)]
pub struct Diagnostic {
  pub(crate) code: &'static str,
  pub(crate) summary: String,
  pub(crate) files: Vec<(String, String)>,
  pub(crate) labels: Vec<Label<(String, Range<usize>)>>,
}

impl Diagnostic {
  fn init_builder(
    code: &'static str,
    message: String,
    labels: Vec<Label<(String, Range<usize>)>>,
  ) -> ReportBuilder<'static, (String, Range<usize>)> {
    let mut builder = Report::<(String, Range<usize>)>::build(ReportKind::Error, "", 0)
      .with_code(code)
      .with_message(message);

    for label in labels {
      builder = builder.with_label(label);
    }

    builder
  }
  pub fn print(self) {
    let builder = Self::init_builder(self.code, self.summary, self.labels);
    builder.finish().print(sources(self.files)).unwrap();
  }

  pub fn print_to_string(self) -> String {
    let builder = Self::init_builder(self.code, self.summary, self.labels);
    let mut output = Vec::new();
    builder
      .with_config(Config::default().with_color(false))
      .finish()
      .write_for_stdout(sources(self.files.clone()), &mut output)
      .unwrap();
    String::from_utf8(output).unwrap()
  }
}

impl Display for Diagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}: {}", self.code, self.summary)
  }
}
