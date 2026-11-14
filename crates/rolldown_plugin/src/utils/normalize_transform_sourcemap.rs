use rolldown_sourcemap::SourceMap;

pub fn normalize_transform_sourcemap(map: SourceMap, id: &str, original_code: &str) -> SourceMap {
  let mut map = map;
  // If sourcemap hasn't `sources`, using original id to fill it.
  let source = map.get_source(0);
  if source.is_none_or(str::is_empty)
    || (map.get_sources().count() == 1 && (source.map(AsRef::as_ref) != Some(id)))
  {
    map.set_sources(vec![id]);
  }
  // If sourcemap hasn't `sourcesContent`, using original code to fill it.
  if map.get_source_content(0).is_none_or(str::is_empty) {
    map.set_source_contents(vec![Some(original_code)]);
  }
  map
}
