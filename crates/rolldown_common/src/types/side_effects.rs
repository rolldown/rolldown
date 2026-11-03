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

  /// `moduleSideEffects` reported to plugins (`boolean | 'no-treeshake'`).
  ///
  /// `Analyzed` is the default inclusion algorithm. Plugins see that as `true`; the
  /// statement-level result stays internal. Keep this aligned with
  /// [`crate::reported_module_side_effects`].
  pub fn to_module_side_effects(self) -> HookSideEffects {
    match self {
      DeterminedSideEffects::UserDefined(false) => HookSideEffects::False,
      DeterminedSideEffects::UserDefined(true) | DeterminedSideEffects::Analyzed(_) => {
        HookSideEffects::True
      }
      DeterminedSideEffects::NoTreeshake => HookSideEffects::NoTreeshake,
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
  pub fn from_resolver(value: &oxc_resolver::SideEffects) -> Self {
    match value {
      oxc_resolver::SideEffects::Bool(v) => SideEffects::Bool(*v),
      oxc_resolver::SideEffects::String(v) => SideEffects::String((*v).to_string()),
      oxc_resolver::SideEffects::Array(v) => {
        SideEffects::Array(v.iter().map(ToString::to_string).collect_vec())
      }
    }
  }
}

pub(crate) fn glob_match_with_normalized_pattern(pattern: &str, path: &str) -> bool {
  let trimmed_str = pattern.trim_start_matches("./");
  let mut normalized_glob = if trimmed_str.len() != pattern.len() {
    String::from("**/") + trimmed_str
  } else if trimmed_str.contains('/') {
    trimmed_str.to_string()
  } else {
    String::from("**/") + trimmed_str
  };
  // `fast_glob` reads a leading `!` as a negation, but `sideEffects` is a positive allowlist and
  // `!foo.js` is a legal path. Escape it so the pattern keeps matching literally.
  if normalized_glob.starts_with('!') {
    normalized_glob.insert(0, '\\');
  }
  fast_glob::glob_match(&normalized_glob, path.trim_start_matches("./"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookSideEffects {
  True,
  False,
  NoTreeshake,
}

#[cfg(test)]
mod tests {
  use super::{DeterminedSideEffects, HookSideEffects};

  #[test]
  fn analyzed_side_effects_are_reported_as_true() {
    assert_eq!(
      DeterminedSideEffects::Analyzed(false).to_module_side_effects(),
      HookSideEffects::True,
    );
    assert_eq!(
      DeterminedSideEffects::Analyzed(true).to_module_side_effects(),
      HookSideEffects::True
    );
    assert_eq!(
      DeterminedSideEffects::UserDefined(false).to_module_side_effects(),
      HookSideEffects::False,
    );
    assert_eq!(
      DeterminedSideEffects::UserDefined(true).to_module_side_effects(),
      HookSideEffects::True,
    );
    assert_eq!(
      DeterminedSideEffects::NoTreeshake.to_module_side_effects(),
      HookSideEffects::NoTreeshake
    );
  }
}
