#[derive(Clone, Debug)]
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

  pub fn derive_side_effects_from_package_json(&self, relative_path: &str) -> bool {
    match self {
      SideEffects::Bool(s) => *s,
      SideEffects::String(s) => glob_match_with_normalized_pattern(s, relative_path),
      SideEffects::Array(patterns) => {
        patterns.iter().any(|pattern| glob_match_with_normalized_pattern(pattern, relative_path))
      }
    }
  }
}

fn glob_match_with_normalized_pattern(pattern: &str, path: &str) -> bool {
  let trimmed_str = pattern.trim_start_matches("./");
  let normalized_glob = if trimmed_str.contains('/') {
    trimmed_str.to_string()
  } else {
    String::from("**/") + trimmed_str
  };
  glob_match::glob_match(&normalized_glob, path.trim_start_matches("./"))
}
