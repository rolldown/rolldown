use oxc::{semantic::SymbolId, span::CompactStr as CompactString};
use rolldown_error::BuildDiagnostic;
use rustc_hash::FxHashMap;

use crate::{AstScopes, ModuleId, ModuleIdx, SymbolRef};

pub const RUNTIME_MODULE_KEY: &str = "\0rolldown/runtime.js";
pub const RUNTIME_MODULE_ID: ModuleId = ModuleId::new_arc_str(arcstr::literal!(RUNTIME_MODULE_KEY));

#[derive(Debug, Clone)]
pub struct RuntimeModuleBrief {
  id: ModuleIdx,
  name_to_symbol: FxHashMap<CompactString, SymbolId>,
  /// Names of plugins that modified the runtime module via the transform hook.
  modified_by_plugins: Vec<String>,
}

impl RuntimeModuleBrief {
  pub fn new(id: ModuleIdx, scope: &AstScopes) -> Self {
    Self {
      id,
      name_to_symbol: scope
        .scoping()
        .get_bindings(scope.scoping().root_scope_id())
        .into_iter()
        .map(|(name, &symbol_id)| (CompactString::new(name), symbol_id))
        .collect(),
      modified_by_plugins: Vec::new(),
    }
  }

  pub fn set_modified_by_plugins(&mut self, plugins: Vec<String>) {
    self.modified_by_plugins = plugins;
  }

  #[inline]
  pub fn id(&self) -> ModuleIdx {
    self.id
  }

  /// Validate that all expected runtime helper symbols are present.
  /// Returns a list of errors for any missing symbols.
  pub fn validate_symbols(&self, expected_symbols: &[&str]) -> Vec<BuildDiagnostic> {
    let missing: Vec<String> = expected_symbols
      .iter()
      .filter(|name| !self.name_to_symbol.contains_key(**name))
      .map(std::string::ToString::to_string)
      .collect();
    if missing.is_empty() {
      vec![]
    } else {
      vec![BuildDiagnostic::runtime_module_symbol_not_found(
        missing,
        self.modified_by_plugins.clone(),
      )]
    }
  }

  pub fn resolve_symbol(&self, name: &str) -> SymbolRef {
    let symbol_id = self.name_to_symbol.get(name).unwrap_or_else(|| {
      panic!(
        "Failed to resolve runtime symbol `{name}`. This should not happen as symbols are validated upfront."
      )
    });
    (self.id, *symbol_id).into()
  }

  pub fn dummy() -> Self {
    Self {
      id: ModuleIdx::new(0),
      name_to_symbol: FxHashMap::default(),
      modified_by_plugins: Vec::new(),
    }
  }
}
