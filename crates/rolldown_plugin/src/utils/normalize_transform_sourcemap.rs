use std::borrow::Cow;

use rolldown_sourcemap::SourceMap;

/// Names the module's own source in a `transform` hook sourcemap, and gives it the module's code
/// when the plugin left its content missing, `null`, or empty.
///
/// A map with at most one source maps back to the transform input, so that source is the module.
/// A map with more sources names its own files (e.g. it was chained from the input's sourcemap),
/// so only an entry named with the module id is the module. The other entries stay as returned.
/// See internal-docs/sourcemap-chain/implementation.md.
pub fn normalize_transform_sourcemap(map: SourceMap, id: &str, original_code: &str) -> SourceMap {
  let mut parts = map.into_parts();
  if parts.sources.len() <= 1 {
    if parts.sources.first().is_none_or(|source| *source != id) {
      parts.sources = vec![Cow::Owned(id.to_owned())];
    }
    fill_empty_content(&mut parts.source_contents, 0, original_code);
  } else {
    for (index, source) in parts.sources.iter().enumerate() {
      if *source == id {
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
  use super::normalize_transform_sourcemap;
  use rolldown_sourcemap::SourceMap;

  const ID: &str = "/project/dep.js";
  const CODE: &str = "console.log('a')\nconsole.log('b')\n";

  fn normalize(json: &'static str) -> (Vec<String>, Vec<Option<String>>) {
    let map = normalize_transform_sourcemap(SourceMap::from_json_string(json).unwrap(), ID, CODE);
    (
      map.get_sources().map(ToOwned::to_owned).collect(),
      map.get_source_contents().map(|content| content.map(ToOwned::to_owned)).collect(),
    )
  }

  fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|&item| item.to_owned()).collect()
  }

  fn contents(items: &[Option<&str>]) -> Vec<Option<String>> {
    items.iter().map(|item| item.map(ToOwned::to_owned)).collect()
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
}
