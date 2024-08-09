use crate::types::generator::GenerateContext;
use rolldown_utils::ecma_script::legitimize_identifier_name;

#[derive(Debug)]
struct ExternalArgument {
  name: String,
  global: String,
  legitimized: String,
  // TODO support `path` field.
  empty: bool,
}

impl ExternalArgument {
  fn as_cjs_import(&self) -> String {
    format!("require('{}');", self.name)
  }

  fn as_amd_import_arg(&self) -> String {
    format!("'{}'", self.name)
  }

  fn as_iife_import(&self) -> Option<String> {
    if self.empty {
      None
    } else {
      Some(self.legitimized.to_string())
    }
  }

  fn as_iife_argument(&self, base: String) -> Option<String> {
    if self.empty {
      None
    } else {
      Some(format!(
        "{}{}",
        if base.is_empty() { String::new() } else { format!("{base}.") },
        self.legitimized
      ))
    }
  }
}

#[derive(Debug)]
struct ExternalArguments {
  arguments: Vec<ExternalArgument>,
}

impl ExternalArguments {
  fn new() -> Self {
    Self { arguments: Vec::new() }
  }

  fn push(&mut self, name: String, empty: bool, ctx: &GenerateContext<'_>) {
    let globals = match &ctx.options.globals.get(name.as_str()) {
      Some(globals) => (*globals).to_string(),
      None => name.to_string(),
    };
    // TODO support `path`.
    let legitimized = legitimize_identifier_name(name.as_str()).to_string();

    let argument = ExternalArgument { name, global: globals, legitimized, empty };

    self.arguments.push(argument);
  }

  fn is_empty(&self) -> bool {
    self.arguments.is_empty()
  }

  fn len(&self) -> usize {
    self.arguments.len()
  }

  fn as_cjs_import_list(&self) -> String {
    self.arguments.iter().map(ExternalArgument::as_cjs_import).collect::<Vec<String>>().join(", ")
  }

  fn as_amd_import_list(&self) -> String {
    format!(
      "[{}]",
      self
        .arguments
        .iter()
        .map(ExternalArgument::as_amd_import_arg)
        .collect::<Vec<String>>()
        .join(", ")
    )
  }

  fn as_iife_import_list(&self, base: String) -> String {}
}
