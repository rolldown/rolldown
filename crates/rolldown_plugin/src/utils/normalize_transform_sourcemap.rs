use std::borrow::Cow;

use rolldown_sourcemap::{SourceMap, source_names_module};

/// Names the module's own source in a `transform` hook sourcemap, and gives it the module's code
/// when the plugin left its content missing, `null`, or empty.
///
/// A map with at most one source maps back to the transform input, so that source is the module.
/// A map with more sources names its own files (e.g. it was chained from the input's sourcemap),
/// so only sources that name the module get the module's content. `source_names_module` checks
/// the module id, absolute path aliases, and relative paths that resolve to the id.
/// See internal-docs/sourcemap-chain/implementation.md.
pub fn normalize_transform_sourcemap(map: SourceMap, id: &str, original_code: &str) -> SourceMap {
  let mut parts = map.into_parts();
  if parts.sources.len() <= 1 {
    if parts.sources.first().is_none_or(|source| source.as_deref() != Some(id)) {
      parts.sources = vec![Some(Cow::Owned(id.to_owned()))];
    }
    fill_empty_content(&mut parts.source_contents, 0, original_code);
  } else {
    let source_root = parts.source_root.as_deref();
    for (index, source) in parts.sources.iter().enumerate() {
      if source.as_deref().is_some_and(|source| source_names_module(source, source_root, id)) {
        fill_empty_content(&mut parts.source_contents, index, original_code);
      }
    }
  }
  SourceMap::from_parts(parts)
}

/// `sourcesContent` can be shorter than `sources`; a missing entry means no content.
fn fill_empty_content(contents: &mut Vec<Option<Cow<'static, str>>>, index: usize, code: &str) {
  if contents.len() <= index {
    contents.resize(index + 1, None);
  }
  if contents[index].as_deref().is_none_or(str::is_empty) {
    contents[index] = Some(Cow::Owned(code.to_owned()));
  }
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use super::normalize_transform_sourcemap;
  use rolldown_sourcemap::SourceMap;

  const ID: &str = "/project/dep.js";
  const CODE: &str = "console.log('a')\nconsole.log('b')\n";

  fn normalize(json: &'static str) -> (Vec<Option<String>>, Vec<Option<String>>) {
    normalize_with_id(json, ID)
  }

  fn normalize_with_id(json: &'static str, id: &str) -> (Vec<Option<String>>, Vec<Option<String>>) {
    let map = normalize_transform_sourcemap(SourceMap::from_json_string(json).unwrap(), id, CODE);
    (
      map.get_sources().map(|source| source.map(ToOwned::to_owned)).collect(),
      map.get_source_contents().map(|content| content.map(ToOwned::to_owned)).collect(),
    )
  }

  /// An absolute module id on every platform, so that relative sources can resolve to it.
  fn absolute_id() -> String {
    std::env::current_dir().unwrap().join("src").join("dep.js").to_str().unwrap().to_owned()
  }

  fn strings(items: &[&str]) -> Vec<Option<String>> {
    items.iter().map(|&item| Some(item.to_owned())).collect()
  }

  fn contents(items: &[Option<&str>]) -> Vec<Option<String>> {
    items.iter().map(|item| item.map(ToOwned::to_owned)).collect()
  }

  #[test]
  fn normalize_transform_sourcemap_preserves_null_sources() {
    for json in [
      r#"{"version":3,"sources":[null,"other.js"],"sourcesContent":[null,"other code"],"names":[],"mappings":"ACAA"}"#,
      r#"{"version":3,"sources":[null,null],"sourcesContent":[null,null],"names":[],"mappings":"AAAA"}"#,
    ] {
      let map = SourceMap::from_json_string(json).unwrap();
      let expected = map.to_json_string();
      let map = normalize_transform_sourcemap(map, "id.js", "original code");
      assert_eq!(map.to_json_string(), expected);
    }
  }

  #[test]
  fn normalize_transform_sourcemap_fills_missing_sources() {
    for json in [
      r#"{"version":3,"sources":[],"sourcesContent":[],"names":[],"mappings":""}"#,
      r#"{"version":3,"sources":[null],"sourcesContent":[null],"names":[],"mappings":"AAAA"}"#,
    ] {
      let map = SourceMap::from_json_string(json).unwrap();
      let map = normalize_transform_sourcemap(map, "id.js", "original code");
      assert_eq!(map.get_source(0), Some("id.js"));
      assert_eq!(map.get_source_content(0), Some("original code"));
    }
  }

  #[test]
  fn single_source_is_the_module() {
    let (sources, sources_content) =
      normalize(r#"{"version":3,"sources":["dep.ts"],"names":[],"mappings":"AAAA"}"#);
    assert_eq!(sources, strings(&[ID]));
    assert_eq!(sources_content, contents(&[Some(CODE)]));

    let (sources, sources_content) = normalize(
      r#"{"version":3,"sources":[""],"sourcesContent":[null],"names":[],"mappings":"AAAA"}"#,
    );
    assert_eq!(sources, strings(&[ID]));
    assert_eq!(sources_content, contents(&[Some(CODE)]));

    // The module's code is known, so an empty string is replaced too.
    let (_, sources_content) = normalize(
      r#"{"version":3,"sources":["dep.ts"],"sourcesContent":[""],"names":[],"mappings":"AAAA"}"#,
    );
    assert_eq!(sources_content, contents(&[Some(CODE)]));
  }

  #[test]
  fn map_without_sources_is_the_module() {
    let (sources, sources_content) =
      normalize(r#"{"version":3,"sources":[],"names":[],"mappings":""}"#);
    assert_eq!(sources, strings(&[ID]));
    assert_eq!(sources_content, contents(&[Some(CODE)]));
  }

  #[test]
  fn single_source_keeps_given_content() {
    let (_, sources_content) = normalize(
      r#"{"version":3,"sources":["dep.ts"],"sourcesContent":["const a = 1"],"names":[],"mappings":"AAAA"}"#,
    );
    assert_eq!(sources_content, contents(&[Some("const a = 1")]));
  }

  /// https://github.com/rolldown/rolldown/issues/11236
  #[test]
  fn multiple_sources_keep_their_content() {
    let (sources, sources_content) = normalize(
      r#"{"version":3,"sources":["/a.ts","/b.ts"],"sourcesContent":["","export const b = 2"],"names":[],"mappings":"AAAA;ACAA"}"#,
    );
    assert_eq!(sources, strings(&["/a.ts", "/b.ts"]));
    assert_eq!(sources_content, contents(&[Some(""), Some("export const b = 2")]));

    let (_, sources_content) = normalize(
      r#"{"version":3,"sources":["/a.ts","/b.ts"],"sourcesContent":[null,"export const b = 2"],"names":[],"mappings":"AAAA;ACAA"}"#,
    );
    assert_eq!(sources_content, contents(&[None, Some("export const b = 2")]));

    // The module's code is not the content of `a.ts`.
    let (_, sources_content) =
      normalize(r#"{"version":3,"sources":["/a.ts","/b.ts"],"names":[],"mappings":"AAAA;ACAA"}"#);
    assert_eq!(sources_content, contents(&[]));
  }

  #[test]
  fn multiple_sources_keep_an_unnamed_source() {
    let (sources, sources_content) = normalize(
      r#"{"version":3,"sources":["","/b.ts"],"sourcesContent":["A","B"],"names":[],"mappings":"AAAA;ACAA"}"#,
    );
    assert_eq!(sources, strings(&["", "/b.ts"]));
    assert_eq!(sources_content, contents(&[Some("A"), Some("B")]));
  }

  #[test]
  fn multiple_sources_fill_only_the_module_entry() {
    let (sources, sources_content) = normalize(
      r#"{"version":3,"sources":["/helper.ts","/project/dep.js"],"sourcesContent":[null],"names":[],"mappings":"AAAA;ACAA"}"#,
    );
    assert_eq!(sources, strings(&["/helper.ts", ID]));
    assert_eq!(sources_content, contents(&[None, Some(CODE)]));

    let (_, sources_content) = normalize(
      r#"{"version":3,"sources":["/helper.ts","/project/dep.js"],"sourcesContent":["",""],"names":[],"mappings":"AAAA;ACAA"}"#,
    );
    assert_eq!(sources_content, contents(&[Some(""), Some(CODE)]));
  }

  /// A relative source names the module when it resolves to the module id, with the rule that
  /// resolves the sources of the first map in the chain.
  #[test]
  fn multiple_sources_fill_a_relative_module_entry() {
    let id = absolute_id();
    for json in [
      r#"{"version":3,"sources":["dep.js","/helper.ts"],"names":[],"mappings":"AAAA;ACAA"}"#,
      r#"{"version":3,"sources":["./dep.js","/helper.ts"],"names":[],"mappings":"AAAA;ACAA"}"#,
      r#"{"version":3,"sources":["../src/dep.js","/helper.ts"],"names":[],"mappings":"AAAA;ACAA"}"#,
      r#"{"version":3,"sourceRoot":"lib","sources":["../dep.js","/helper.ts"],"names":[],"mappings":"AAAA;ACAA"}"#,
    ] {
      let (sources, sources_content) = normalize_with_id(json, &id);
      assert_eq!(sources_content, contents(&[Some(CODE)]), "{json}");
      // The sources stay as returned. The collapse resolves them later.
      assert_eq!(sources[1].as_deref(), Some("/helper.ts"), "{json}");
    }

    let (sources, sources_content) = normalize_with_id(
      r#"{"version":3,"sources":["/helper.ts","./dep.js"],"sourcesContent":["H",""],"names":[],"mappings":"AAAA;ACAA"}"#,
      &id,
    );
    assert_eq!(sources, strings(&["/helper.ts", "./dep.js"]));
    assert_eq!(sources_content, contents(&[Some("H"), Some(CODE)]));
  }

  #[test]
  fn multiple_sources_fill_an_absolute_module_alias_in_the_second_entry() {
    let id = absolute_id();
    let dir = Path::new(&id).parent().unwrap();
    let alias = dir.join("other").join("..").join("dep.js");
    let alias = alias.to_str().unwrap();
    let mut map = SourceMap::from_json_string(
      r#"{"version":3,"sources":["helper.ts","dep.js"],"names":[],"mappings":"AAAA;ACAA"}"#,
    )
    .unwrap();
    map.set_sources([Some("helper.ts"), Some(alias)]);
    let map = normalize_transform_sourcemap(map, &id, CODE);
    assert_eq!(map.get_source(1), Some(alias));
    assert_eq!(map.get_source_content(0), None);
    assert_eq!(map.get_source_content(1), Some(CODE));
  }

  #[test]
  fn multiple_sources_skip_a_relative_entry_of_another_file() {
    let id = absolute_id();
    for json in [
      r#"{"version":3,"sources":["lib/dep.js","/helper.ts"],"names":[],"mappings":"AAAA;ACAA"}"#,
      r#"{"version":3,"sourceRoot":"lib","sources":["dep.js","/helper.ts"],"names":[],"mappings":"AAAA;ACAA"}"#,
      // A URL `sourceRoot` is not a directory.
      r#"{"version":3,"sourceRoot":"webpack://pkg/","sources":["dep.js","/helper.ts"],"names":[],"mappings":"AAAA;ACAA"}"#,
    ] {
      let (_, sources_content) = normalize_with_id(json, &id);
      assert_eq!(sources_content, contents(&[]), "{json}");
    }

    // A virtual module has no directory, so only its id names it.
    let (_, sources_content) = normalize_with_id(
      r#"{"version":3,"sources":["virtual","/helper.ts"],"names":[],"mappings":"AAAA;ACAA"}"#,
      "\0virtual",
    );
    assert_eq!(sources_content, contents(&[]));
  }
}
