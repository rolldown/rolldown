use rolldown_sourcemap::SourceMap;

pub fn normalize_transform_sourcemap(map: SourceMap, id: &str, original_code: &str) -> SourceMap {
  let mut map = map;
  // Single-source transforms may omit the input name; multi-source maps retain null slots.
  let source_count = map.get_sources().len();
  let source = map.get_source(0);
  if source_count == 0
    || source.is_some_and(str::is_empty)
    || (source_count == 1 && source.is_none_or(|source| source != id))
  {
    map.set_sources(vec![Some(id)]);
  }
  // If sourcemap hasn't `sourcesContent`, using original code to fill it.
  if map.get_source(0).is_some() && map.get_source_content(0).is_none_or(str::is_empty) {
    map.set_source_contents(vec![Some(original_code)]);
  }
  map
}

#[cfg(test)]
mod tests {
  use super::normalize_transform_sourcemap;
  use rolldown_sourcemap::SourceMap;

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
}
