// cSpell:disable
pub use concat_sourcemap::{ConcatSource, RawSource, Source, SourceMapSource};
pub use oxc::sourcemap::SourceMapBuilder;
use oxc::sourcemap::Token;
pub use oxc::sourcemap::{JSONSourceMap, SourceMap, SourcemapVisualizer};
mod lines_count;
pub use lines_count::lines_count;
mod concat_sourcemap;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

#[allow(clippy::from_iter_instead_of_collect, clippy::cast_possible_truncation)]
pub fn collapse_sourcemaps(mut sourcemap_chain: Vec<&SourceMap>) -> SourceMap {
  debug_assert!(sourcemap_chain.len() > 1);
  let last_map = sourcemap_chain.pop().expect("sourcemap_chain should not be empty");
  let first_map = sourcemap_chain.first().expect("sourcemap_chain should not be empty");

  let sourcemap_and_lookup_table = sourcemap_chain
    .par_iter()
    .map(|sourcemap| (sourcemap, sourcemap.generate_lookup_table()))
    .collect::<Vec<_>>();

  let source_view_tokens = last_map.get_source_view_tokens().collect::<Vec<_>>();

  let names_map =
    FxHashMap::from_iter(first_map.get_names().enumerate().map(|(i, name)| (name, i as u32)));

  let sources_map =
    FxHashMap::from_iter(first_map.get_sources().enumerate().map(|(i, source)| (source, i as u32)));

  let tokens = source_view_tokens
    .par_iter()
    .filter_map(|token| {
      let original_token = sourcemap_and_lookup_table.iter().rev().try_fold(
        *token,
        |token, (sourcemap, lookup_table)| {
          sourcemap.lookup_source_view_token(
            lookup_table,
            token.get_src_line(),
            token.get_src_col(),
          )
        },
      );
      original_token.map(|original_token| {
        Token::new(
          token.get_dst_line(),
          token.get_dst_col(),
          original_token.get_src_line(),
          original_token.get_src_col(),
          original_token
            .get_source_id()
            .and_then(|source_id| first_map.get_source(source_id))
            .and_then(|source| sources_map.get(source).copied()),
          original_token
            .get_name_id()
            .and_then(|name_id| first_map.get_name(name_id))
            .and_then(|name| names_map.get(name).copied()),
        )
      })
    })
    .collect::<Vec<_>>();

  SourceMap::new(
    None,
    first_map.get_names().map(Into::into).collect::<Vec<_>>(),
    None,
    first_map.get_sources().map(Into::into).collect::<Vec<_>>(),
    first_map.get_source_contents().map(|x| x.map(Into::into).collect::<Vec<_>>()),
    tokens,
    None,
  )
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
      let map = super::collapse_sourcemaps(sourcemaps.iter().collect::<Vec<_>>());
      map.to_json_string().unwrap()
    };

    let expected = "{\"version\":3,\"names\":[],\"sources\":[\"/project/foo.js\"],\"sourcesContent\":[\"\\n\\n  1 + 1;\"],\"mappings\":\"AAEE\"}";

    assert_eq!(&result, expected);
  }
}
