use oxc::span::CompactStr;

/// We only concatenate wrapped modules when `WrapKind` is `Esm`
#[derive(Debug, Clone, Copy, Default)]
pub enum ConcatenateWrappedModuleKind {
  /// The len of module group of the module should be greater equal than 2
  /// The module is the root of the module group
  Root,
  /// Just a normal esm wrapped module  
  #[default]
  None,
  /// The len of module group of the module should be greater equal than 2
  /// The module is the inner module of the module group
  Inner,
}

#[derive(Debug, Clone, Default)]
pub struct RenderedConcatenatedModuleParts {
  pub hoisted_vars: Vec<CompactStr>,
  pub hoisted_functions_or_module_ns_decl: Vec<String>,
  pub wrap_ref_name: Option<CompactStr>,
  pub rendered_esm_runtime_expr: Option<String>,
}
