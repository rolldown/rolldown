use crate::types::generator::GenerateContext;
use arcstr::ArcStr;
use rolldown_error::BuildDiagnostic;
use rolldown_utils::ecma_script::legitimize_identifier_name;

#[derive(Debug)]
pub struct External {
  /// The imported name.
  pub name: String,
  /// If it is like `import "..."` or `require('...')`, without any imported variables.
  pub empty: bool,
}

impl External {
  #[allow(dead_code)]
  pub fn as_cjs(&self) -> String {
    // TODO support `options.path`.
    format!("require('{}')", self.name)
  }

  #[allow(dead_code)]
  pub fn as_amd(&self) -> String {
    // TODO support `options.path`.
    // We just need to wrap it with the `'`, as we should use the array in module list.
    format!("'{}'", self.name)
  }

  pub fn as_argument(&self) -> Option<String> {
    if self.empty {
      None
    } else {
      Some(legitimize_identifier_name(&self.name).to_string())
    }
  }

  pub fn as_iife(&self, ctx: &mut GenerateContext<'_>) -> Option<String> {
    if self.empty {
      None
    } else if let Some(global) = ctx.options.globals.get(&self.name) {
      Some(global.to_string())
    } else {
      let fallback = self.as_argument()?;
      ctx.warnings.push(
        BuildDiagnostic::missing_global_name(
          ArcStr::from(self.name.as_str()),
          ArcStr::from(fallback.as_str()),
        )
        .with_severity_warning(),
      );
      Some(fallback)
    }
  }
}

#[derive(Debug)]
pub struct ExternalModules {
  pub modules: Vec<External>,
}

impl ExternalModules {
  pub fn new() -> Self {
    Self { modules: Vec::new() }
  }

  pub fn push(&mut self, name: String, empty: bool) {
    self.modules.push(External { name, empty });
  }

  /// Only used in UMD, for generating the list of `require`.
  #[allow(dead_code)]
  pub fn as_cjs(&self) -> String {
    self.modules.iter().map(External::as_cjs).collect::<Vec<String>>().join(", ")
  }

  #[allow(dead_code)]
  pub fn as_amd(&self, named_export: bool) -> String {
    let base = if named_export { vec!["exports".to_string()] } else { Vec::new() };
    let args = base
      .into_iter()
      .chain(self.modules.iter().map(External::as_amd))
      .collect::<Vec<String>>()
      .join(", ");
    format!("[{args}]")
  }

  pub fn as_args(&self, named_export: bool) -> String {
    let base = if named_export { vec!["exports".to_string()] } else { Vec::new() };
    base
      .into_iter()
      .chain(self.modules.iter().filter_map(External::as_argument))
      .collect::<Vec<String>>()
      .join(", ")
  }

  pub fn as_iife(&self, ctx: &mut GenerateContext<'_>, exports: &str) -> String {
    let base = if exports.is_empty() { Vec::new() } else { vec![exports.to_string()] };
    base
      .into_iter()
      .chain(self.modules.iter().map(|m| m.as_iife(ctx).unwrap_or_default()))
      .collect::<Vec<String>>()
      .join(", ")
  }
}
