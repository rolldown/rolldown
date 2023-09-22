use rustc_hash::FxHashMap;

#[derive(Debug)]
pub enum ModuleSideEffects {
  Bool(bool),
  NoTreeShake,
  Null,
}

#[derive(Debug)]
pub enum SyntheticNamedExports {
  Bool(bool),
  String(String),
  Null,
}

pub type Assertions = FxHashMap<String, String>;

pub enum SourceResult {
  String(String),
  Object(Box<SourceDescription>),
}

pub struct SourceDescription {
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
