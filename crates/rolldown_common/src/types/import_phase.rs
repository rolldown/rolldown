/// The phase of an import. See https://github.com/tc39/proposal-defer-import-eval.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportPhase {
  /// `import x from 'mod'` and `import('mod')`: links and evaluates the module.
  #[default]
  Evaluation,
  /// `import defer * as ns from 'mod'` and `import.defer('mod')`: links the module, and
  /// evaluates it at the first access to the namespace.
  Defer,
}

impl ImportPhase {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Evaluation => "evaluation",
      Self::Defer => "defer",
    }
  }
}

impl TryFrom<&str> for ImportPhase {
  type Error = String;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "evaluation" => Ok(Self::Evaluation),
      "defer" => Ok(Self::Defer),
      _ => Err(format!("Invalid import phase: {value}")),
    }
  }
}
