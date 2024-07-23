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

#[test]
fn test_collapse_sourcemaps() {
  use crate::{collapse_sourcemaps, ConcatSource, SourceMapSource};
  use oxc::{
    allocator::Allocator,
    codegen::{CodeGenerator, CodegenReturn},
    parser::Parser,
    sourcemap::SourcemapVisualizer,
    span::SourceType,
  };

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
  let ret2: oxc::parser::ParserReturn = Parser::new(&allocator, &source_text, source_type).parse();
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

  let map = collapse_sourcemaps(sourcemap_chain);
  assert_eq!(
    SourcemapVisualizer::new(&source_text, &map).into_visualizer_text(),
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
}
