use ariadne::{
  sources, ColorGenerator, Config, Fmt, Label, Report, ReportBuilder, ReportKind, Span,
};
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

#[test]
fn main() {
  let mut colors = ColorGenerator::new();

  // Generate some colours for each of our elements
  let a = colors.next();
  let b = colors.next();
  let c = colors.next();

  Report::build(ReportKind::Error, "", 10)
    .with_code(3)
    .with_message(format!("Cannot add types Nat and Str"))
    .with_label(
      Label::new(("b.tao", 10..14))
        .with_message(format!("This is of type {}", "Nat".fg(a)))
        .with_color(a),
    )
    .with_label(
      Label::new(("b.tao", 17..20))
        .with_message(format!("This is of type {}", "Str".fg(b)))
        .with_color(b),
    )
    .with_label(
      Label::new(("b.tao", 15..16))
        .with_message(format!(" {} and {} undergo addition here", "Nat".fg(a), "Str".fg(b)))
        .with_color(c)
        .with_order(10),
    )
    .with_label(
      Label::new(("a.tao", 4..8))
        .with_message(format!("Original definition of {} is here", "five".fg(a)))
        .with_color(a),
    )
    .with_note(format!("{} is a number and can only be added to other numbers", "Nat".fg(a)))
    .finish()
    .print(sources(vec![("a.tao", "s".repeat(50)), ("b.tao", "s".repeat(50))]))
    .unwrap();
}
