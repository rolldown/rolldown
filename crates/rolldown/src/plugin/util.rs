use rustc_hash::FxHashMap;

#[derive(Debug)]
pub enum ModuleSideEffects {
  Bool(bool),
  NoTreeShake,
}

#[derive(Debug)]
pub enum SyntheticNamedExports {
  Bool(bool),
  String(String),
}

pub type Assertions = FxHashMap<String, String>;

pub struct SourceResult {
  pub code: String,
  pub map: Option<SourceMapValue>,
  // ast
  // pub assertions: Option<FxHashMap<String, String>>,
  // pub meta: Option<FxHashMap<String, String>>,
  pub module_side_effects: Option<ModuleSideEffects>,
  pub synthetic_named_exports: Option<SyntheticNamedExports>,
}

pub enum SourceMapValue {
  String(String),
  Object(FxHashMap<String, String>),
}
