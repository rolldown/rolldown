#[derive(Debug, Copy, Clone)]
/// represent the side-effects of module is derived from package.json or analyzed from source file
pub enum DeterminedSideEffects {
  PackageJson(bool),
  Analyzed(bool),
}

impl DeterminedSideEffects {
  /// get the boolean value of the side effects enum
  pub fn has_side_effects(&self) -> bool {
    match self {
      DeterminedSideEffects::PackageJson(v) | DeterminedSideEffects::Analyzed(v) => *v,
    }
  }
}

#[derive(Clone, Debug)]
/// A field in `package.json`
pub enum SideEffects {
  Bool(bool),
  String(String),
  Array(Vec<String>),
}

impl SideEffects {
  pub fn from_description(description: &serde_json::Value) -> Option<Self> {
    description.get("sideEffects").and_then(|value| match value {
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
    })
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
  glob_match::glob_match(&normalized_glob, path.trim_start_matches("./"))
}
