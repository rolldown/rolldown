use oxc_resolver::NODEJS_BUILTINS;
use rolldown_utils::{js_regex::HybridRegex, pattern_filter::StringOrRegex};
use rustc_hash::FxHashSet;

const NODE_BUILTIN_NAMESPACE: &str = "node:";
const BUN_BUILTIN_NAMESPACE: &str = "bun:";

#[derive(Debug)]
pub struct BuiltinChecker {
  builtin_strings: FxHashSet<String>,
  builtin_regexes: Vec<HybridRegex>,
}

impl BuiltinChecker {
  pub fn new(builtins: Vec<StringOrRegex>) -> Self {
    let mut builtin_strings = FxHashSet::default();
    let mut builtin_regexes = Vec::new();
    for builtin in builtins {
      match builtin {
        StringOrRegex::String(s) => {
          builtin_strings.insert(s);
        }
        StringOrRegex::Regex(regex) => {
          builtin_regexes.push(regex);
        }
      }
    }
    Self { builtin_strings, builtin_regexes }
  }

  pub fn is_builtin(&self, id: &str) -> bool {
    if self.builtin_strings.contains(id) {
      return true;
    }
    self.builtin_regexes.iter().any(|regex| regex.matches(id))
  }
}

pub fn is_node_like_builtin(id: &str) -> bool {
  id.starts_with(BUN_BUILTIN_NAMESPACE)
    || id.starts_with(NODE_BUILTIN_NAMESPACE)
    || NODEJS_BUILTINS.contains(&id)
}
