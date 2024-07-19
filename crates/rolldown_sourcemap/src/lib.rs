// cSpell:disable
pub use concat_sourcemap::{ConcatSource, RawSource, Source, SourceMapSource};
pub use oxc::sourcemap::SourceMapBuilder;
pub use oxc::sourcemap::{JSONSourceMap, SourceMap, SourcemapVisualizer};
use oxc::sourcemap::{Token, TokenChunk};
mod lines_count;
pub use lines_count::lines_count;
mod concat_sourcemap;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

#[allow(
  clippy::from_iter_instead_of_collect,
  clippy::cast_possible_truncation,
  clippy::too_many_lines
)]
pub fn collapse_sourcemaps(
  mut sourcemap_chain: Vec<&SourceMap>,
  enable_token_chunks: bool,
) -> SourceMap {
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

  let token_chunks = if enable_token_chunks {
    let mut token_chunks = vec![];
    let mut start = 0_u32;
    let mut pre_chunk_last_valid_name_id = 0;
    let mut next_chunk_last_valid_name_id = 0;
    let mut pre_chunk_last_dst_line = 0;
    let mut pre_chunk_last_dst_col = 0;
    let mut pre_chunk_last_src_line = 0;
    let mut pre_chunk_last_src_col = 0;
    let mut pre_token_source_id = u32::MAX;
    let mut pre_chunk_last_valid_source_id = 0;

    for (i, token) in tokens.iter().enumerate() {
      if let Some(source_id) = token.get_source_id() {
        if pre_token_source_id != source_id && pre_token_source_id != u32::MAX {
          token_chunks.push(TokenChunk::new(
            start,
            i as u32,
            pre_chunk_last_dst_line,
            pre_chunk_last_dst_col,
            pre_chunk_last_src_line,
            pre_chunk_last_src_col,
            pre_chunk_last_valid_name_id,
            pre_chunk_last_valid_source_id,
          ));
          start = i as u32;
          pre_chunk_last_valid_name_id = next_chunk_last_valid_name_id;
          pre_chunk_last_valid_source_id = pre_token_source_id;
          if let Some(pre_token) = tokens.get(i - 1) {
            pre_chunk_last_dst_line = pre_token.get_dst_line();
            pre_chunk_last_dst_col = pre_token.get_dst_col();
            pre_chunk_last_src_line = pre_token.get_src_line();
            pre_chunk_last_src_col = pre_token.get_src_col();
          }
        }
        pre_token_source_id = source_id;
      }

      if let Some(name_id) = token.get_name_id() {
        next_chunk_last_valid_name_id = name_id;
      }
    }

    token_chunks.push(TokenChunk::new(
      start,
      tokens.len() as u32,
      pre_chunk_last_dst_line,
      pre_chunk_last_dst_col,
      pre_chunk_last_src_line,
      pre_chunk_last_src_col,
      pre_chunk_last_valid_name_id,
      pre_chunk_last_valid_source_id,
    ));

    Some(token_chunks)
  } else {
    None
  };

  SourceMap::new(
    None,
    first_map.get_names().map(Into::into).collect::<Vec<_>>(),
    None,
    first_map.get_sources().map(Into::into).collect::<Vec<_>>(),
    first_map.get_source_contents().map(|x| x.map(Into::into).collect::<Vec<_>>()),
    tokens,
    token_chunks,
  )
}

#[cfg(test)]
mod tests {
  use crate::{collapse_sourcemaps, ConcatSource, SourceMapSource};
  use oxc::{
    allocator::Allocator,
    codegen::{CodeGenerator, CodegenReturn},
    parser::Parser,
    sourcemap::SourcemapVisualizer,
    span::SourceType,
  };

  #[test]
  fn it_works() {
    let allocator = Allocator::default();

    let mut concat_source = ConcatSource::default();

    let filename = "foo.js".to_string();
    let source_text = "const foo = 1; console.log(foo);\n".to_string();
    let source_type = SourceType::from_path(&filename).unwrap();
    let ret1 = Parser::new(&allocator, &source_text, source_type).parse();
    let CodegenReturn { source_map, source_text } =
      CodeGenerator::new().enable_source_map(&filename, &source_text).build(&ret1.program);
    concat_source.add_source(Box::new(SourceMapSource::new(
      source_text.clone(),
      source_map.unwrap(),
      source_text.matches('\n').count() as u32,
    )));

    let filename = "bar.js".to_string();
    let source_text = "const bar = 2; console.log(bar);\n".to_string();
    let ret2: oxc::parser::ParserReturn =
      Parser::new(&allocator, &source_text, source_type).parse();
    let CodegenReturn { source_map, source_text } =
      CodeGenerator::new().enable_source_map(&filename, &source_text).build(&ret2.program);
    concat_source.add_source(Box::new(SourceMapSource::new(
      source_text.clone(),
      source_map.unwrap(),
      source_text.matches('\n').count() as u32,
    )));

    let (source_text, source_map) = concat_source.content_and_sourcemap();

    let mut sourcemap_chain = vec![];

    sourcemap_chain.push(source_map.as_ref().unwrap());

    let filename = "chunk.js".to_string();
    let ret3 = Parser::new(&allocator, &source_text, source_type).parse();
    let CodegenReturn { source_map, source_text } =
      CodeGenerator::new().enable_source_map(&filename, &source_text).build(&ret3.program);
    sourcemap_chain.push(source_map.as_ref().unwrap());

    let map1 = collapse_sourcemaps(sourcemap_chain.clone(), false);
    assert_eq!(
      SourcemapVisualizer::new(&source_text, &map1).into_visualizer_text(),
      r#"- foo.js
(0:0-0:6) "const " --> (0:0-0:6) "const "
(0:6-0:12) "foo = " --> (0:6-0:12) "foo = "
(0:12-0:15) "1; " --> (0:12-1:0) "1;"
(0:15-0:23) "console." --> (1:0-1:8) "\nconsole"
(0:23-0:27) "log(" --> (1:8-1:12) ".log"
(0:27-0:31) "foo)" --> (1:12-1:16) "(foo"
(0:31-1:1) ";\n" --> (1:16-2:0) ");"
- bar.js
(0:0-0:6) "const " --> (2:0-2:6) "\nconst"
(0:6-0:12) "bar = " --> (2:6-2:12) " bar ="
(0:12-0:15) "2; " --> (2:12-3:0) " 2;"
(0:15-0:23) "console." --> (3:0-3:8) "\nconsole"
(0:23-0:27) "log(" --> (3:8-3:12) ".log"
(0:27-0:31) "bar)" --> (3:12-3:16) "(bar"
(0:31-1:1) ";\n" --> (3:16-4:1) ");\n"
"#
    );

    let map2 = collapse_sourcemaps(sourcemap_chain, true);
    assert_eq!(map1.to_json().mappings, map2.to_json().mappings);
  }
}
