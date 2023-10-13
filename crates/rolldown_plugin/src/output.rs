#[derive(Debug)]
pub struct ResolveIdResult {
  pub id: String,
  pub external: Option<bool>,
  //   pub assertions: Option<FxHashMap<String, String>>,
  //   pub meta: Option<FxHashMap<String, String>>,
  //   pub module_side_effects: Option<ModuleSideEffects>,
  //   pub resolved_by: Option<String>,
  //   pub synthetic_named_exports: Option<SyntheticNamedExports>,
}

// #[derive(Debug)]
// pub enum ResolveIdExternal {
//   Bool(bool),
//     Absolute,
//     Relative,
// }

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

#[derive(Debug)]
pub struct SourceResult {
  pub code: String,
  //   pub map: Option<SourceMapValue>,
  // ast
  // pub assertions: Option<FxHashMap<String, String>>,
  // pub meta: Option<FxHashMap<String, String>>,
  // pub module_side_effects: Option<ModuleSideEffects>,
  // pub synthetic_named_exports: Option<SyntheticNamedExports>,
}

// #[derive(Debug)]
// pub enum SourceMapValue {
//   String(String),
//   Object(FxHashMap<String, String>),
// }
