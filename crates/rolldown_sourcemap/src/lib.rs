pub use sourcemap::SourceMap;
mod concat_sourcemap;

pub use concat_sourcemap::concat_sourcemaps;
use rolldown_error::BuildError;
use sourcemap::SourceMapBuilder;

pub fn collapse_sourcemaps(sourcemap_chain: &[SourceMap]) -> Result<Option<SourceMap>, BuildError> {
  let Some(last_map) = sourcemap_chain.last() else { return Ok(None) };

  let mut sourcemap_builder = SourceMapBuilder::new(None);

  if let Some(map) = sourcemap_chain.first() {
    for source in map.sources() {
      let source_id = sourcemap_builder.add_source(source);
      sourcemap_builder.set_source_contents(source_id, map.get_source_contents(source_id));
    }
    sourcemap_builder.set_source_root(map.get_source_root());
  }

  for token in last_map.tokens() {
    let original_token = sourcemap_chain.iter().rev().try_fold(token, |token, sourcemap| {
      sourcemap.lookup_token(token.get_src_line(), token.get_src_col())
    });

    if let Some(original_token) = original_token {
      token.get_name().map(|name| sourcemap_builder.add_name(name));

      sourcemap_builder.add(
        token.get_dst_line(),
        token.get_dst_col(),
        original_token.get_src_line(),
        original_token.get_src_col(),
        original_token.get_source(),
        original_token.get_name(),
      );
    }
  }

  Ok(Some(sourcemap_builder.into_sourcemap()))
}

#[cfg(test)]
mod tests {
  use crate::SourceMap;
  #[test]
  fn it_works() {
    let sourcemaps = vec![
      SourceMap::from_slice(
        r#"{
        "mappings": ";CAEE",
        "names": [],
        "sources": ["helloworld.js"],
        "sourcesContent": ["\n\n  1 + 1;"],
        "version": 3,
        "ignoreList": []
      }"#
          .as_bytes(),
      )
      .unwrap(),
      SourceMap::from_slice(
        r#"{
        "file": "transpiled.min.js",
        "mappings": "AACCA",
        "names": ["add"],
        "sources": ["transpiled.js"],
        "sourcesContent": ["1+1"],
        "version": 3,
        "ignoreList": []
      }"#
          .as_bytes(),
      )
      .unwrap(),
    ];

    let result = {
      let map = super::collapse_sourcemaps(&sourcemaps).expect("should not fail").unwrap();
      let mut buf = vec![];
      map.to_writer(&mut buf).unwrap();
      unsafe { String::from_utf8_unchecked(buf) }
    };

    let expected = "{\"version\":3,\"sources\":[\"helloworld.js\"],\"sourcesContent\":[\"\\n\\n  1 + 1;\"],\"names\":[\"add\"],\"mappings\":\"AAEE\"}";

    assert_eq!(&result, expected);
  }
}
