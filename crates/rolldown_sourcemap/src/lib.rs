mod source;
mod source_joiner;

use std::sync::Arc;

use oxc_sourcemap::Token;
use rustc_hash::FxHashMap;

pub use oxc_sourcemap::{JSONSourceMap, SourceMap, SourceMapBuilder, SourcemapVisualizer};
pub use source_joiner::SourceJoiner;

pub use crate::source::{Source, SourceMapSource};

use rolldown_utils::rustc_hash::FxHashMapExt;

/// Filter out tokens with invalid source positions (beyond source content bounds).
/// 
/// Invalid tokens can be generated when:
/// 1. oxc_codegen adds punctuation (semicolons, newlines) and creates sourcemap tokens
///    that reference positions beyond the original source line ends
/// 2. Rolldown's PreProcessor generates unique AST spans at positions beyond the source
///    (program.span.end + 1) for deduplication, which oxc_codegen then uses
///
/// This is a workaround until oxc_codegen is fixed to not generate tokens for positions
/// beyond the source content bounds.
fn is_token_valid(token: &Token, sourcemap: &SourceMap) -> bool {
  let Some(source_id) = token.get_source_id() else {
    return true; // Tokens without source_id are considered valid
  };
  
  let Some(source_content) = sourcemap.get_source_content(source_id) else {
    return true; // If no source content, we can't validate, so assume valid
  };
  
  // Split source into lines and check if the token's position is valid
  let source_lines: Vec<&str> = source_content.split('\n').collect();
  let src_line = token.get_src_line() as usize;
  let src_col = token.get_src_col() as usize;
  
  if src_line >= source_lines.len() {
    return false;
  }
  
  let line_content = source_lines[src_line];
  // Calculate UTF-16 length of the line
  let line_len_utf16: usize = line_content.chars().map(|c| c.len_utf16()).sum();
  
  src_col < line_len_utf16
}

/// Remove invalid tokens from a sourcemap.
///
/// This function filters out sourcemap tokens that have source positions beyond the
/// actual source content bounds. See `is_token_valid()` for details on why these
/// invalid tokens are generated.
pub fn filter_invalid_tokens(sourcemap: SourceMap) -> SourceMap {
  let valid_tokens: Vec<Token> = sourcemap
    .get_tokens()
    .filter(|token| is_token_valid(&token, &sourcemap))
    .collect();
  
  SourceMap::new(
    sourcemap.get_file().map(Arc::clone),
    sourcemap.get_names().map(Arc::clone).collect(),
    None,
    sourcemap.get_sources().map(Arc::clone).collect(),
    sourcemap.get_source_contents().map(|x| x.map(Arc::clone)).collect(),
    valid_tokens.into_boxed_slice(),
    None,
  )
}

// <https://github.com/rollup/rollup/blob/master/src/utils/collapseSourcemaps.ts>
#[expect(clippy::cast_possible_truncation)]
pub fn collapse_sourcemaps(sourcemap_chain: &[&SourceMap]) -> SourceMap {
  debug_assert!(sourcemap_chain.len() > 1);
  if sourcemap_chain.len() == 1 {
    // If there's only one sourcemap, return it as is.
    return sourcemap_chain[0].clone();
  }

  let last_map = sourcemap_chain.last().expect("sourcemap_chain should not be empty");
  let first_map = sourcemap_chain.first().expect("sourcemap_chain should not be empty");
  let chain_without_last = &sourcemap_chain[..sourcemap_chain.len() - 1];

  let sourcemap_and_lookup_table = chain_without_last
    .iter()
    .map(|sourcemap| (sourcemap, sourcemap.generate_lookup_table()))
    .collect::<Vec<_>>();

  let source_view_tokens = last_map.get_source_view_tokens();

  let sources_map = first_map
    .get_sources()
    .enumerate()
    .map(|(i, source)| (source, i as u32))
    .collect::<FxHashMap<_, _>>();

  // Avoid hashing the source text for every token.
  let mut sources_cache = FxHashMap::with_capacity(sources_map.len());

  let tokens = source_view_tokens
    .filter_map(|token| {
      let original_token = sourcemap_and_lookup_table.iter().rev().try_fold(
        token,
        |token, (sourcemap, lookup_table)| {
          sourcemap.lookup_source_view_token(
            lookup_table,
            token.get_src_line(),
            token.get_src_col(),
          )
        },
      );
      original_token.and_then(|original_token| {
        // Validate that the source position is within bounds of the source content
        let source_id = original_token.get_source_id()?;
        let source_content = first_map.get_source_content(source_id)?;
        
        // Split source into lines and check if the token's position is valid
        let source_lines: Vec<&str> = source_content.split('\n').collect();
        let src_line = original_token.get_src_line() as usize;
        let src_col = original_token.get_src_col() as usize;
        
        if src_line >= source_lines.len() {
          return None;
        }
        
        let line_content = source_lines[src_line];
        // Calculate UTF-16 length of the line
        let line_len_utf16: usize = line_content.chars().map(|c| c.len_utf16()).sum();
        
        if src_col >= line_len_utf16 {
          return None;
        }
        
        Some(Token::new(
          token.get_dst_line(),
          token.get_dst_col(),
          original_token.get_src_line(),
          original_token.get_src_col(),
          original_token.get_source_id().and_then(|source_id| {
            sources_cache
              .entry(source_id)
              .or_insert_with(|| {
                first_map.get_source(source_id).and_then(|source| sources_map.get(source))
              })
              .copied()
          }),
          original_token.get_name_id(),
        ))
      })
    })
    .collect::<Vec<_>>();

  SourceMap::new(
    None,
    first_map.get_names().map(Arc::clone).collect::<Vec<_>>(),
    None,
    first_map.get_sources().map(Arc::clone).collect::<Vec<_>>(),
    first_map.get_source_contents().map(|x| x.map(Arc::clone)).collect::<Vec<_>>(),
    tokens.into_boxed_slice(),
    None,
  )
}

#[test]
fn test_collapse_sourcemaps() {
  use crate::{SourceJoiner, SourceMapSource, collapse_sourcemaps};
  use oxc::{
    allocator::Allocator,
    codegen::{Codegen, CodegenOptions, CodegenReturn, CommentOptions},
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
  let CodegenReturn { map, code, .. } = Codegen::new()
    .with_options(CodegenOptions {
      comments: CommentOptions { normal: false, ..CommentOptions::default() },
      source_map_path: Some(filename.into()),
      ..CodegenOptions::default()
    })
    .build(&ret1.program);
  source_joiner.append_source(SourceMapSource::new(code, map.as_ref().unwrap().clone()));

  let filename = "bar.js".to_string();
  let source_text = "const bar = 2; console.log(bar);\n".to_string();
  let ret2: oxc::parser::ParserReturn = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { map, code, .. } = Codegen::new()
    .with_options(CodegenOptions {
      source_map_path: Some(filename.into()),
      ..CodegenOptions::default()
    })
    .build(&ret2.program);
  source_joiner.append_source(SourceMapSource::new(code, map.as_ref().unwrap().clone()));

  let (source_text, source_map) = source_joiner.join();

  let mut sourcemap_chain = vec![];

  sourcemap_chain.push(source_map.as_ref().unwrap());

  let filename = "chunk.js".to_string();
  let ret3 = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { map, code, .. } = Codegen::new()
    .with_options(CodegenOptions {
      comments: CommentOptions { normal: false, ..CommentOptions::default() },
      source_map_path: Some(filename.into()),
      ..CodegenOptions::default()
    })
    .build(&ret3.program);
  sourcemap_chain.push(map.as_ref().unwrap());

  let map = collapse_sourcemaps(&sourcemap_chain);
  assert_eq!(
    SourcemapVisualizer::new(&code, &map).get_text(),
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
