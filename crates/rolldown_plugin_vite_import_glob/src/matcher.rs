use arcstr::ArcStr;
use globstar::{CompileOptions, Glob, GlobError};

use crate::ViteImportGlobPlugin;

impl ViteImportGlobPlugin {
  /// `id` is a slash-normalized module id.
  pub(crate) fn set_globs(&self, id: &str, matchers: Vec<GlobMatcher>) {
    if matchers.is_empty() {
      self.glob_matchers.remove(id);
    } else {
      self.glob_matchers.insert(ArcStr::from(id), matchers);
    }
  }
}

/// Tells whether a file is matched by one `import.meta.glob` call, like the matcher vite builds
/// with picomatch in its `vite:import-glob` plugin.
///
/// See `internal-docs/import-meta-glob/design.md`.
#[derive(Debug)]
pub struct GlobMatcher {
  affirmed: Glob,
  negated: Option<Glob>,
}

impl GlobMatcher {
  /// The globs are absolute.
  pub fn new(
    affirmed: Vec<String>,
    mut negated: Vec<String>,
    exhaustive: bool,
    case_sensitive: bool,
  ) -> Result<Self, GlobError> {
    let options = CompileOptions::default().dot(exhaustive).case_insensitive(!case_sensitive);
    if !exhaustive {
      negated.push("**/node_modules/**".to_string());
    }
    Ok(Self {
      affirmed: Glob::union_with(affirmed, options)?,
      negated: if negated.is_empty() { None } else { Some(Glob::union_with(negated, options)?) },
    })
  }

  /// `file` is a slash-normalized absolute path.
  pub fn is_match(&self, file: &str) -> bool {
    let file = file.as_bytes();
    self.affirmed.is_match(file) && !self.negated.as_ref().is_some_and(|glob| glob.is_match(file))
  }
}

#[cfg(test)]
mod tests {
  use arcstr::ArcStr;
  use rolldown_common::WatcherChangeKind::{self, Create, Delete, Update};
  use rolldown_plugin::{HookHotUpdateArgs, Plugin as _, PluginContext};

  use super::GlobMatcher;
  use crate::{ViteImportGlobPlugin, utils::PathWithGlob};

  fn globs(globs: &[&str]) -> Vec<String> {
    globs.iter().map(ToString::to_string).collect()
  }

  fn matcher(affirmed: &[&str], negated: &[&str]) -> GlobMatcher {
    GlobMatcher::new(globs(affirmed), globs(negated), false, true).unwrap()
  }

  #[test]
  fn matches_a_single_level_pattern() {
    let m = matcher(&["/p/src/pages/*.js"], &[]);
    assert!(m.is_match("/p/src/pages/a.js"));
    assert!(!m.is_match("/p/src/pages/a.ts"));
    // `*` does not cross a separator.
    assert!(!m.is_match("/p/src/pages/nested/a.js"));
    assert!(!m.is_match("/p/src/pages-legacy/a.js"));
    assert!(!m.is_match("/p/src/pages"));
  }

  #[test]
  fn matches_a_globstar_pattern_across_levels() {
    let m = matcher(&["/p/src/pages/**/*.js"], &[]);
    assert!(m.is_match("/p/src/pages/a.js"));
    assert!(m.is_match("/p/src/pages/nested/deep/a.js"));
    assert!(!m.is_match("/p/src/pages/nested/a.css"));
  }

  #[test]
  fn matches_any_of_the_affirmed_patterns() {
    let m = matcher(&["/p/a/*.js", "/p/b/**/*.{ts,tsx}"], &[]);
    assert!(m.is_match("/p/a/x.js"));
    assert!(m.is_match("/p/b/c/x.tsx"));
    assert!(!m.is_match("/p/c/x.js"));
  }

  #[test]
  fn honors_negated_patterns() {
    let m = matcher(&["/p/src/pages/*.js"], &["/p/src/pages/*.test.js"]);
    assert!(m.is_match("/p/src/pages/a.js"));
    assert!(!m.is_match("/p/src/pages/a.test.js"));
  }

  #[test]
  fn skips_dot_entries_and_node_modules() {
    let m = matcher(&["/p/src/**/*.js"], &[]);
    assert!(m.is_match("/p/src/a.js"));
    assert!(!m.is_match("/p/src/.cache/a.js"));
    assert!(!m.is_match("/p/src/.hidden.js"));
    assert!(!m.is_match("/p/src/node_modules/dep/a.js"));
    // A dot directory the glob names is matched as written.
    let m = matcher(&["/p/.storybook/*.js"], &[]);
    assert!(m.is_match("/p/.storybook/a.js"));
  }

  #[test]
  fn exhaustive_keeps_dot_entries_and_node_modules() {
    let m = GlobMatcher::new(globs(&["/p/src/**/*.js"]), Vec::new(), true, true).unwrap();
    assert!(m.is_match("/p/src/.cache/a.js"));
    assert!(m.is_match("/p/src/node_modules/dep/a.js"));
  }

  #[test]
  fn folds_case_when_case_sensitive_is_off() {
    let sensitive = matcher(&["/p/src/*.JS"], &[]);
    assert!(!sensitive.is_match("/p/src/a.js"));

    let insensitive = GlobMatcher::new(globs(&["/p/src/*.JS"]), Vec::new(), false, false).unwrap();
    assert!(insensitive.is_match("/p/src/a.js"));
  }

  #[test]
  fn rejects_a_glob_that_does_not_compile() {
    GlobMatcher::new(globs(&["/p/src/[a"]), Vec::new(), false, true).unwrap_err();
  }

  #[test]
  fn escapes_the_glob_syntax_of_the_literal_path() {
    // `./*.js` written next to `/p/[lang]/{x}/main.js`
    let glob = PathWithGlob::new("/p/[lang]/{x}/*.js".to_string(), "./*.js").to_absolute_glob();
    assert_eq!(glob, r"/p/\[lang\]/\{x\}/*.js");
    let m = matcher(&[&glob], &[]);
    assert!(m.is_match("/p/[lang]/{x}/a.js"));
    assert!(!m.is_match("/p/l/x/a.js"));

    // What the user wrote stays a glob.
    let glob = PathWithGlob::new("/p/src/[ab]/*.js".to_string(), "./[ab]/*.js").to_absolute_glob();
    assert_eq!(glob, "/p/src/[ab]/*.js");
  }

  #[test]
  fn matches_a_glob_without_a_pattern() {
    let glob = PathWithGlob::new("/p/src/page.js".to_string(), "./page.js").to_absolute_glob();
    let m = matcher(&[&glob], &[]);
    assert!(m.is_match("/p/src/page.js"));
    assert!(!m.is_match("/p/src/page.jsx"));
  }

  fn plugin(owners: &[&str]) -> ViteImportGlobPlugin {
    let plugin = ViteImportGlobPlugin::default();
    for owner in owners {
      plugin.set_globs(owner, vec![matcher(&["/p/pages/*.js"], &[])]);
    }
    plugin
  }

  fn hot_update(
    plugin: &ViteImportGlobPlugin,
    kind: WatcherChangeKind,
    file: &str,
    modules: &[&str],
  ) -> Option<Vec<ArcStr>> {
    let modules = modules.iter().copied().map(ArcStr::from).collect();
    let args = HookHotUpdateArgs { kind, file: file.into(), modules };
    let ctx = PluginContext::new_napi_context();
    futures::executor::block_on(plugin.hot_update(&ctx, &args)).unwrap()
  }

  #[test]
  fn adds_the_owners_in_a_stable_order() {
    let plugin = plugin(&["/p/b.js", "/p/a.js"]);
    assert_eq!(
      hot_update(&plugin, Create, "/p/pages/c.js", &["/p/pages/c.js"]).unwrap(),
      ["/p/pages/c.js", "/p/a.js", "/p/b.js"]
    );
    assert_eq!(hot_update(&plugin, Delete, "/p/pages/c.js", &[]).unwrap(), ["/p/a.js", "/p/b.js"]);
  }

  #[test]
  fn declines_what_no_glob_matches() {
    let plugin = plugin(&["/p/main.js"]);
    assert_eq!(hot_update(&plugin, Update, "/p/pages/a.js", &["/p/pages/a.js"]), None);
    assert_eq!(hot_update(&plugin, Create, "/p/pages/notes.txt", &[]), None);
    // A deleted directory is reported as itself.
    assert_eq!(hot_update(&plugin, Delete, "/p/pages", &[]), None);
  }
}
