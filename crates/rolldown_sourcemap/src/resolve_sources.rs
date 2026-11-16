use std::path::Path;

use oxc_sourcemap::SourceMapParts;
use sugar_path::SugarPath;

use crate::SourceMap;

/// Resolves the relative `sources` of a plugin map against the module:
/// `resolve(dirname(module_id), sourceRoot, source)`.
///
/// Rollup does this for the `load` hook map.
/// Rolldown also keeps the sources of a multi-source `transform` hook map,
/// so it applies the rule to the first map of the chain.
///
/// Returns `None` when nothing changes.
/// A `sourceRoot` that was applied is cleared.
pub fn resolve_sources_against_module(map: &SourceMap, module_id: &str) -> Option<SourceMap> {
  let module_path = Path::new(module_id);
  if !module_path.is_absolute() {
    return None;
  }
  let module_dir = module_path.parent()?;
  let source_root = map.get_source_root().filter(|root| !root.is_empty());
  if source_root.is_some_and(is_url) || !map.get_sources().any(is_relative_path) {
    return None;
  }
  let base = match source_root {
    Some(root) => module_dir.join(root),
    None => module_dir.to_path_buf(),
  };
  let sources = map
    .get_sources()
    .map(|source| {
      if is_relative_path(source) {
        base.join(source).normalize().to_string_lossy().into_owned()
      } else {
        source.to_string()
      }
    })
    .collect::<Vec<_>>();

  let mut map = map.clone();
  map.set_sources(sources);
  if source_root.is_some() {
    let parts = map.into_parts();
    map = SourceMap::from_parts(SourceMapParts { source_root: None, ..parts });
  }
  Some(map)
}

fn is_url(source: &str) -> bool {
  source.contains("://") || source.starts_with("data:")
}

fn is_relative_path(source: &str) -> bool {
  !source.is_empty()
    && !source.starts_with('\0')
    && !source.starts_with('/')
    && !is_url(source)
    && !Path::new(source).is_absolute()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn map(sources: &[&str], source_root: Option<&str>) -> SourceMap {
    SourceMap::new(
      None,
      vec![],
      source_root.map(|root| root.to_string().into()),
      sources.iter().map(|s| (*s).to_string().into()).collect(),
      vec![],
      Box::new([]),
      None,
    )
  }

  fn sources(map: &SourceMap) -> Vec<&str> {
    map.get_sources().collect()
  }

  #[cfg(not(windows))]
  #[test]
  fn resolves_relative_sources_against_module_dir() {
    let resolved = resolve_sources_against_module(
      &map(&["../src/tags.ts", "/abs/a.ts", "webpack://pkg/b.ts", "purify.es.mjs"], None),
      "/work/node_modules/dompurify/dist/purify.es.mjs",
    )
    .unwrap();
    assert_eq!(
      sources(&resolved),
      vec![
        "/work/node_modules/dompurify/src/tags.ts",
        "/abs/a.ts",
        "webpack://pkg/b.ts",
        "/work/node_modules/dompurify/dist/purify.es.mjs",
      ]
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn applies_and_clears_source_root() {
    let resolved =
      resolve_sources_against_module(&map(&["a.ts"], Some("../src/")), "/pkg/dist/x.js").unwrap();
    assert_eq!(sources(&resolved), vec!["/pkg/src/a.ts"]);
    assert_eq!(resolved.get_source_root(), None);

    let resolved =
      resolve_sources_against_module(&map(&["a.ts"], Some("")), "/pkg/dist/x.js").unwrap();
    assert_eq!(sources(&resolved), vec!["/pkg/dist/a.ts"]);
    assert_eq!(resolved.get_source_root(), Some(""));
  }

  #[test]
  fn returns_none_when_nothing_changes() {
    // A virtual module has no directory to resolve against.
    assert!(resolve_sources_against_module(&map(&["a.ts"], None), "\0virtual").is_none());
    // A URL `sourceRoot` is not a directory.
    assert!(
      resolve_sources_against_module(&map(&["a.ts"], Some("webpack://pkg/")), "/pkg/x.js")
        .is_none()
    );
    // No source is relative.
    assert!(resolve_sources_against_module(&map(&["/pkg/x.js"], None), "/pkg/x.js").is_none());
  }
}
