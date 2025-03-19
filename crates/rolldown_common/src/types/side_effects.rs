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
  pub fn from_json_value(value: &serde_json::Value) -> Option<Self> {
    match value {
      serde_json::Value::Bool(v) => Some(SideEffects::Bool(*v)),
      serde_json::Value::String(v) => Some(SideEffects::String(v.to_string())),
      serde_json::Value::Array(v) => {
        let mut side_effects = vec![];
        for value in v {
          let str = value.as_str()?;
          side_effects.push(str.to_string());
        }
        Some(SideEffects::Array(side_effects))
      }
      _ => None,
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
