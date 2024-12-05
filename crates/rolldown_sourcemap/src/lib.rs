// cSpell:disable
pub use oxc_sourcemap::SourceMapBuilder;
use oxc_sourcemap::Token;
pub use oxc_sourcemap::{JSONSourceMap, SourceMap, SourcemapVisualizer};
pub use source_joiner::SourceJoiner;
mod lines_count;
pub use lines_count::lines_count;
mod source_joiner;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;
mod source;

pub use crate::source::{Source, SourceMapSource};

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
  use crate::{collapse_sourcemaps, SourceJoiner, SourceMapSource};
  use oxc::{
    allocator::Allocator,
    codegen::{CodeGenerator, CodegenOptions, CodegenReturn},
    parser::Parser,
    span::SourceType,
  };
  use oxc_sourcemap::SourcemapVisualizer;

  let allocator = Allocator::default();

  let mut source_joiner = SourceJoiner::default();

  let filename = "foo.js".to_string();
  let source_text = "const foo = 1; console.log(foo);\n".to_string();
  let source_type = SourceType::from_path(&filename).unwrap();
  let ret1 = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { map, code, .. } = CodeGenerator::new()
    .with_options(CodegenOptions {
      source_map_path: Some(filename.into()),
      ..CodegenOptions::default()
    })
    .build(&ret1.program);
  source_joiner.append_source(SourceMapSource::new(code.clone(), map.as_ref().unwrap().clone()));

  let filename = "bar.js".to_string();
  let source_text = "const bar = 2; console.log(bar);\n".to_string();
  let ret2: oxc::parser::ParserReturn = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { map, code, .. } = CodeGenerator::new()
    .with_options(CodegenOptions {
      source_map_path: Some(filename.into()),
      ..CodegenOptions::default()
    })
    .build(&ret2.program);
  source_joiner.append_source(SourceMapSource::new(code.clone(), map.as_ref().unwrap().clone()));

  let (source_text, source_map) = source_joiner.join();

  let mut sourcemap_chain = vec![];

  sourcemap_chain.push(source_map.as_ref().unwrap());

  let filename = "chunk.js".to_string();
  let ret3 = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { map, code, .. } = CodeGenerator::new()
    .with_options(CodegenOptions {
      source_map_path: Some(filename.into()),
      ..CodegenOptions::default()
    })
    .build(&ret3.program);
  sourcemap_chain.push(map.as_ref().unwrap());

  let map = collapse_sourcemaps(sourcemap_chain);
  assert_eq!(
    SourcemapVisualizer::new(&code, &map).into_visualizer_text(),
    r#"- foo.js
(0:0) "const " --> (0:0) "const "
(0:6) "foo = " --> (0:6) "foo = "
(0:12) "1; " --> (0:12) "1;\n"
(0:15) "console." --> (1:0) "console."
(0:23) "log(" --> (1:8) "log("
(0:27) "foo)" --> (1:12) "foo)"
(0:31) ";\n" --> (1:16) ";\n"
- bar.js
(0:0) "const " --> (2:0) "const "
(0:6) "bar = " --> (2:6) "bar = "
(0:12) "2; " --> (2:12) "2;\n"
(0:15) "console." --> (3:0) "console."
(0:23) "log(" --> (3:8) "log("
(0:27) "bar)" --> (3:12) "bar)"
(0:31) ";\n" --> (3:16) ";\n"
"#
  );
}
