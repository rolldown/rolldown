use itertools::Itertools;

#[derive(Debug, Copy, Clone)]
/// represent the side-effects of module is derived from `side effect hook`, package.json or analyzed from source file
pub enum DeterminedSideEffects {
  /// Including the side effects from package.json and hook side effects
  UserDefined(bool),
  Analyzed(bool),
  /// Align with rollup moduleSideEffects
  NoTreeshake,
}

impl DeterminedSideEffects {
  /// get the boolean value of the side effects enum
  pub fn has_side_effects(&self) -> bool {
    match self {
      DeterminedSideEffects::UserDefined(v) | DeterminedSideEffects::Analyzed(v) => *v,
      DeterminedSideEffects::NoTreeshake => true,
    }
  }
}

#[derive(Debug, Clone)]
/// A field in `package.json`
pub enum SideEffects {
  Bool(bool),
  String(String),
  Array(Vec<String>),
}

impl SideEffects {
  pub fn from_resolver(value: &oxc_resolver::SideEffects) -> Option<Self> {
    match value {
      oxc_resolver::SideEffects::Bool(v) => Some(SideEffects::Bool(*v)),
      oxc_resolver::SideEffects::String(v) => Some(SideEffects::String((*v).to_string())),
      oxc_resolver::SideEffects::Array(v) => {
        Some(SideEffects::Array(v.iter().map(ToString::to_string).collect_vec()))
      }
    }
  }
}

pub(crate) fn glob_match_with_normalized_pattern(pattern: &str, path: &str) -> bool {
  let trimmed_str = pattern.trim_start_matches("./");
  let normalized_glob = if trimmed_str.len() != pattern.len() {
    String::from("**/") + trimmed_str
  } else if trimmed_str.contains('/') {
    trimmed_str.to_string()
  } else {
    String::from("**/") + trimmed_str
  };
  fast_glob::glob_match(&normalized_glob, path.trim_start_matches("./"))
}

#[derive(Debug, Clone, Copy)]
pub enum HookSideEffects {
  True,
  False,
  NoTreeshake,
}
