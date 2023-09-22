use rolldown_common::RawPath;

use super::util::{ModuleSideEffects, SyntheticNamedExports};

#[derive(Debug)]
pub struct ResolveIdArgsOptions {
  //   pub assertions: FxHashMap<String, String>,
  pub is_entry: bool,
  // pub custom
}

#[derive(Debug)]
pub struct ResolveIdArgs<'a> {
  pub importer: Option<&'a RawPath>,
  pub source: &'a str,
  pub options: ResolveIdArgsOptions,
}

#[derive(Debug)]
pub enum ResolveIdResult {
  String(String),
  False, // external module
  Object(Box<PartialResolvedId>),
}

impl ResolveIdResult {
  pub fn is_external(&self) -> bool {
    match self {
      ResolveIdResult::False => true,
      ResolveIdResult::Object(obj) => match &obj.external {
        None => false,
        Some(x) => x.is_external(),
      },
      _ => false,
    }
  }
}

#[derive(Debug)]
pub enum ResolveIdExternal {
  Bool(bool),
  Absolute,
  Relative,
}

impl ResolveIdExternal {
  pub fn is_external(&self) -> bool {
    match self {
      ResolveIdExternal::Bool(b) => *b,
      _ => true,
    }
  }
}

#[derive(Debug)]
pub struct PartialResolvedId {
  pub id: String,
  pub external: Option<ResolveIdExternal>,
  //   pub assertions: Option<FxHashMap<String, String>>,
  //   pub meta: Option<FxHashMap<String, String>>,
  pub module_side_effects: Option<ModuleSideEffects>,
  pub resolved_by: Option<String>,
  pub synthetic_named_exports: Option<SyntheticNamedExports>,
}
