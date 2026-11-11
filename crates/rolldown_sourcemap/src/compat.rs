use oxc_sourcemap::{SourceMap, SourceMapParts, Token, TokenChunk};

/// Adapts the maps returned by Oxc 0.158 until Oxc adopts sourcemap v10.
///
/// Move the strings and convert the numeric tokens directly, without encoding and decoding
/// the mappings. Consuming the vectors also lets `collect` reuse their allocations.
pub fn from_oxc_sourcemap(map: oxc_sourcemap_v9::SourceMap<'_>) -> SourceMap<'_> {
  let parts = map.into_parts();
  SourceMap::from_parts(SourceMapParts {
    file: parts.file,
    names: parts.names,
    source_root: parts.source_root,
    sources: parts.sources.into_iter().map(Some).collect(),
    source_contents: parts.source_contents,
    tokens: parts
      .tokens
      .into_vec()
      .into_iter()
      .map(|token| {
        Token::new(
          token.get_dst_line(),
          token.get_dst_col(),
          token.get_src_line(),
          token.get_src_col(),
          token.get_source_id(),
          token.get_name_id(),
        )
      })
      .collect::<Vec<_>>()
      .into_boxed_slice(),
    token_chunks: parts.token_chunks.map(|chunks| {
      chunks
        .into_iter()
        .map(|chunk| {
          TokenChunk::new(
            chunk.start,
            chunk.end,
            chunk.prev_dst_line,
            chunk.prev_dst_col,
            chunk.prev_src_line,
            chunk.prev_src_col,
            chunk.prev_name_id,
            chunk.prev_source_id,
          )
        })
        .collect()
    }),
    ignore_list: parts.ignore_list,
    debug_id: parts.debug_id,
  })
}

#[test]
fn conversion_preserves_mappings_and_owned_buffers() {
  let source = String::from("input.js");
  let content = String::from("let input = 1;");
  let source_ptr = source.as_ptr();
  let content_ptr = content.as_ptr();
  let tokens = vec![
    oxc_sourcemap_v9::Token::new(0, 2, 0, 4, Some(0), Some(0)),
    oxc_sourcemap_v9::Token::new(0, 8, 0, 0, None, None),
  ]
  .into_boxed_slice();
  let tokens_ptr = tokens.as_ptr().cast::<u8>();
  let sources: Vec<std::borrow::Cow<'_, str>> = vec![source.into()];
  let sources_ptr = sources.as_ptr().cast::<u8>();
  let token_chunks = vec![
    oxc_sourcemap_v9::TokenChunk::new(0, 1, 0, 0, 0, 0, 0, 0),
    oxc_sourcemap_v9::TokenChunk::new(1, 2, 0, 2, 0, 4, 0, 0),
  ];
  let chunks_ptr = token_chunks.as_ptr().cast::<u8>();
  let map = oxc_sourcemap_v9::SourceMap::from_parts(oxc_sourcemap_v9::SourceMapParts {
    file: Some("output.js".into()),
    names: vec!["input".into()],
    source_root: Some("src".into()),
    sources,
    source_contents: vec![Some(content.into())],
    tokens,
    token_chunks: Some(token_chunks),
    ignore_list: Some(vec![0]),
    debug_id: Some("map-id".into()),
  });
  let expected = map.to_json_string();
  let converted = from_oxc_sourcemap(map);
  assert_eq!(converted.to_json_string(), expected);
  let parts = converted.into_parts();
  assert_eq!(parts.sources[0].as_ref().unwrap().as_ptr(), source_ptr);
  assert_eq!(parts.source_contents[0].as_ref().unwrap().as_ptr(), content_ptr);
  assert_eq!(parts.tokens.as_ptr().cast::<u8>(), tokens_ptr);
  assert_eq!(parts.sources.as_ptr().cast::<u8>(), sources_ptr);
  let chunks = parts.token_chunks.unwrap();
  assert_eq!(chunks.as_ptr().cast::<u8>(), chunks_ptr);
  let chunk = &chunks[1];
  assert_eq!((chunk.start, chunk.end, chunk.prev_dst_col, chunk.prev_src_col), (1, 2, 2, 4));
}

#[test]
fn conversion_keeps_borrowed_contents() {
  let content = String::from("let input = 1;");
  let mut builder = oxc_sourcemap_v9::SourceMapBuilder::default();
  let source = builder.add_source_and_content("input.js", &content);
  builder.add_token(0, 0, 0, 0, Some(source), None);
  let map = from_oxc_sourcemap(builder.into_sourcemap());
  assert_eq!(map.get_source_content(0).unwrap().as_ptr(), content.as_ptr());
  assert!(matches!(
    map.into_parts().source_contents[0].as_ref(),
    Some(std::borrow::Cow::Borrowed(_))
  ));
}
