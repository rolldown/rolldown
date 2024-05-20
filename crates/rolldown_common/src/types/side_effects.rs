use globset::{Glob, GlobSet, GlobSetBuilder};

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

  pub(crate) fn global_matcher(&self) -> GlobSet {
    let mut set_builder = GlobSetBuilder::new();
    match self {
      SideEffects::Bool(_) => {}
      SideEffects::String(pat) => {
        set_builder.add(Glob::new(&normalized_pattern(pat)).unwrap());
      }
      SideEffects::Array(pats) => {
        for pat in pats {
          set_builder.add(Glob::new(&normalized_pattern(pat)).unwrap());
        }
      }
    };
    set_builder.build().unwrap()
  }
}

/// Possible patterns in `sideEffects` field:
/// - `./src/some-side-effect-file.js`
/// - `*.css`
/// - `**/*.css`
/// - `./esnext/index.js`
fn normalized_pattern(pattern: &str) -> String {
  // For relative path, remove leading `./`, and add `**/` prefix.
  // `./src/foo.js` -> `**/src/foo.js`.
  let trimmed_str = pattern.trim_start_matches("./");
  if trimmed_str.len() == pattern.len() {
    trimmed_str.to_string()
  } else {
    format!("**/{trimmed_str}")
  }
}

#[test]
fn test_side_effects() {
  use serde_json::json;
  let side_effects = SideEffects::from_description(&json!({
    "sideEffects": [
      "./ke?p/*/file.js",
      "./remove/this/file.j",
      "./re?ve/this/file.js"
    ]
  }));

  let side_effects = side_effects.unwrap();

  assert!(side_effects.global_matcher().is_match("node_modules/demo-pkg/keep/this/file.js"),);
}
