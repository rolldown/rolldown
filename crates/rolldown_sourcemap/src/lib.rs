// cSpell:disable
pub use concat_sourcemap::{ConcatSource, RawSource, Source, SourceMapSource};
pub use oxc::sourcemap::SourceMapBuilder;
pub use oxc::sourcemap::{JSONSourceMap, SourceMap, SourcemapVisualizer};
mod lines_count;
pub use lines_count::lines_count;
mod concat_sourcemap;

pub fn collapse_sourcemaps(mut sourcemap_chain: Vec<&SourceMap>) -> Option<SourceMap> {
  debug_assert!(sourcemap_chain.len() > 1);
  let last_map = sourcemap_chain.pop()?;

  let mut sourcemap_builder = SourceMapBuilder::default();

  let sourcemap_and_lookup_table = sourcemap_chain
    .iter()
    .map(|sourcemap| (sourcemap, sourcemap.generate_lookup_table()))
    .collect::<Vec<_>>();

  for token in last_map.get_source_view_tokens() {
    let original_token = sourcemap_and_lookup_table.iter().rev().try_fold(
      token,
      |token, (sourcemap, lookup_table)| {
        sourcemap.lookup_source_view_token(lookup_table, token.get_src_line(), token.get_src_col())
      },
    );

    if let Some(original_token) = original_token {
      token
        .get_name_id()
        .and_then(|id| last_map.get_name(id))
        .map(|name| sourcemap_builder.add_name(name));

      let name_id = original_token.get_name().map(|name| sourcemap_builder.add_name(name));

      let source_id = original_token.get_source_and_content().map(|(source, source_content)| {
        sourcemap_builder.add_source_and_content(source, source_content)
      });

      sourcemap_builder.add_token(
        token.get_dst_line(),
        token.get_dst_col(),
        original_token.get_src_line(),
        original_token.get_src_col(),
        source_id,
        name_id,
      );
    }
  }

  Some(sourcemap_builder.into_sourcemap())
}

#[cfg(test)]
mod tests {
  use crate::SourceMap;
  #[test]
  fn it_works() {
    let sourcemaps = vec![
      SourceMap::from_json_string(
        r#"{
        "mappings": ";CAEE",
        "names": [],
        "sources": ["/project/foo.js"],
        "sourcesContent": ["\n\n  1 + 1;"],
        "version": 3,
        "ignoreList": []
      }"#,
      )
      .unwrap(),
      SourceMap::from_json_string(
        r#"{
        "file": "transpiled.min.js",
        "mappings": "AACCA",
        "names": ["add"],
        "sources": ["/project/foo_transform.js"],
        "sourcesContent": ["1+1"],
        "version": 3,
        "ignoreList": []
      }"#,
      )
      .unwrap(),
    ];

    let result = {
      let map = super::collapse_sourcemaps(sourcemaps.iter().collect::<Vec<_>>()).unwrap();
      map.to_json_string().unwrap()
    };

    let expected = "{\"version\":3,\"names\":[\"add\"],\"sources\":[\"project/foo.js\"],\"sourcesContent\":[\"\\n\\n  1 + 1;\"],\"mappings\":\"AAEE\"}";

    assert_eq!(&result, expected);
  }
}
