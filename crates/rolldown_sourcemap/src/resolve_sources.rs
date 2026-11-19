use std::{borrow::Cow, path::Path};

use oxc_sourcemap::SourceMapParts;
use sugar_path::{SugarPath, SugarPathBuf};

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
  let source_root = map.get_source_root().filter(|root| !root.is_empty());
  let base = source_base(module_id, source_root)?;
  if !map.get_sources().flatten().any(is_relative_path) {
    return None;
  }
  let sources = map
    .get_sources()
    .map(|source| {
      source.map(|source| {
        if is_relative_path(source) {
          base.join(source).normalize().to_string_lossy().into_owned()
        } else {
          source.to_string()
        }
      })
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

/// Returns whether a source names the module. It can be the module id, an absolute path that
/// normalizes to it, or a relative path that [`resolve_sources_against_module`] resolves to it.
pub fn source_names_module(source: &str, source_root: Option<&str>, module_id: &str) -> bool {
  if source == module_id {
    return true;
  }
  let module_path = Path::new(module_id);
  let source_path = Path::new(source);
  if source_path.is_absolute() {
    return module_path.is_absolute() && source_path.normalize().as_ref() == module_path;
  }
  is_relative_path(source)
    && source_base(module_id, source_root)
      .is_some_and(|base| base.join(source_path).into_normalized() == module_path)
}

/// The directory that relative sources resolve against: `resolve(dirname(module_id), sourceRoot)`.
///
/// Returns `None` when the rule does not apply: the module id is not an absolute path, or
/// `sourceRoot` is a URL.
fn source_base<'a>(module_id: &'a str, source_root: Option<&str>) -> Option<Cow<'a, Path>> {
  let module_path = Path::new(module_id);
  if !module_path.is_absolute() {
    return None;
  }
  let module_dir = module_path.parent()?;
  match source_root.filter(|root| !root.is_empty()) {
    Some(root) if is_url(root) => None,
    Some(root) => Some(Cow::Owned(module_dir.join(root))),
    None => Some(Cow::Borrowed(module_dir)),
  }
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
      sources.iter().map(|s| Some((*s).to_string().into())).collect(),
      vec![],
      Box::new([]),
      None,
    )
  }

  fn sources(map: &SourceMap) -> Vec<&str> {
    map.get_sources().flatten().collect()
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
  fn preserves_null_source_slots() {
    let mut input = map(&["a.ts"], Some("src"));
    input.set_sources([None, Some("a.ts")]);
    let dir = std::env::current_dir().unwrap();
    let module_id = dir.join("out.js");
    let resolved = resolve_sources_against_module(&input, module_id.to_str().unwrap()).unwrap();
    let sources = resolved.get_sources().collect::<Vec<_>>();
    assert_eq!(sources[0], None);
    let expected = dir.join("src").join("a.ts");
    assert_eq!(sources[1], expected.to_str());
    assert_eq!(resolved.get_source_root(), None);
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

  #[test]
  fn source_names_module_with_the_resolve_rule() {
    let id = std::env::current_dir().unwrap().join("src").join("x.js");
    let id = id.to_str().unwrap();
    for (source, root) in [
      (id, None),
      (id, Some("lib")),
      ("x.js", None),
      ("./x.js", None),
      ("../src/x.js", None),
      ("x.js", Some("")),
      ("../x.js", Some("lib")),
    ] {
      assert!(source_names_module(source, root, id), "{source} {root:?}");
    }
    for (source, root) in [
      ("y.js", None),
      ("lib/x.js", None),
      ("x.js", Some("lib")),
      // A URL `sourceRoot` is not a directory.
      ("x.js", Some("webpack://pkg/")),
      ("webpack://pkg/x.js", None),
      ("", None),
    ] {
      assert!(!source_names_module(source, root, id), "{source} {root:?}");
    }
    // A virtual module has no directory, so only its id names it.
    assert!(source_names_module("\0virtual", None, "\0virtual"));
    assert!(!source_names_module("virtual", None, "\0virtual"));
  }

  #[test]
  fn source_names_module_matches_absolute_path_aliases() {
    let dir = std::env::current_dir().unwrap().join("src");
    let id = dir.join("x.js");
    let id = id.to_str().unwrap();
    let dir = dir.to_str().unwrap();
    let sep = std::path::MAIN_SEPARATOR;
    for source in [
      format!("{dir}{sep}.{sep}x.js"),
      format!("{dir}{sep}{sep}x.js"),
      format!("{dir}{sep}other{sep}..{sep}x.js"),
    ] {
      assert!(source_names_module(&source, None, id), "{source}");
      assert!(!source_names_module(&source, None, &format!("{id}?query")), "{source}");
    }
    let other = format!("{dir}{sep}.{sep}y.js");
    assert!(!source_names_module(&other, None, id));
  }
}
