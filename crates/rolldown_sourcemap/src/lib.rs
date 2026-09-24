mod source;
mod source_joiner;

use std::borrow::Cow;

use oxc_sourcemap::Token;

pub use oxc_sourcemap::{JSONSourceMap, OwnedSourceMap, SourceMapBuilder, SourcemapVisualizer};
pub use source_joiner::SourceJoiner;

/// Rolldown always stores and produces owned sourcemaps, so we alias the
/// lifetime-parameterized `oxc_sourcemap::SourceMap` to its `'static` form.
pub type SourceMap = oxc_sourcemap::SourceMap<'static>;

pub use crate::source::{Source, SourceMapSource};

/// Strips the first `lines` destination lines from the sourcemap, decrementing all remaining
/// destination line numbers accordingly. Used to re-anchor a sourcemap after removing a
/// prefix (e.g. a shebang line) from the generated code.
///
/// Reuses the map's existing allocations: strings are moved, and the token buffer is mutated
/// in place unless tokens have to be dropped (only when some token maps into the stripped
/// prefix, which rolldown's own maps never do — prepended text carries no tokens).
pub fn adjust_sourcemap_dst_lines(sourcemap: SourceMap, lines: u32) -> SourceMap {
  if lines == 0 {
    return sourcemap;
  }

  let shift = |token: &Token| {
    Token::new(
      token.get_dst_line() - lines,
      token.get_dst_col(),
      token.get_src_line(),
      token.get_src_col(),
      token.get_source_id(),
      token.get_name_id(),
    )
  };

  let mut parts = sourcemap.into_parts();
  if parts.tokens.iter().any(|t| t.get_dst_line() < lines) {
    parts.tokens = parts.tokens.iter().filter(|t| t.get_dst_line() >= lines).map(shift).collect();
  } else {
    for token in &mut parts.tokens {
      *token = shift(token);
    }
  }
  // The chunk boundaries and VLQ baselines in `token_chunks` describe the pre-shift tokens.
  parts.token_chunks = None;

  SourceMap::from_parts(parts)
}

/// Builds an empty sourcemap with no tokens, sources, names, or contents.
pub fn empty_sourcemap() -> SourceMap {
  SourceMap::new(None, vec![], None, vec![], vec![], Box::new([]), None)
}

// <https://github.com/rollup/rollup/blob/master/src/utils/collapseSourcemaps.ts>
//
// Input maps may borrow their strings (e.g. a codegen map borrowing the source it was
// printed from) — the output is always owned, since it copies every string it keeps.
// This lets callers collapse a freshly generated map without `into_owned`-ing it first.
pub fn collapse_sourcemaps(sourcemap_chain: &[&oxc_sourcemap::SourceMap<'_>]) -> SourceMap {
  debug_assert!(sourcemap_chain.len() > 1);
  if sourcemap_chain.len() == 1 {
    // If there's only one sourcemap, return it as is.
    return sourcemap_chain[0].clone().into_owned();
  }

  let last_map = sourcemap_chain.last().expect("sourcemap_chain should not be empty");
  let first_map = sourcemap_chain.first().expect("sourcemap_chain should not be empty");
  let chain_without_last = &sourcemap_chain[..sourcemap_chain.len() - 1];

  // Concatenate each map's names into one pool, in chain order. A token's merged name_id is then
  // `local_id + offset`, where `offset` is the pool index of that map's first name.
  let mut merged_names: Vec<Cow<'static, str>> = Vec::new();
  let mut append_names = |map: &oxc_sourcemap::SourceMap<'_>| {
    #[expect(clippy::cast_possible_truncation)]
    let offset = merged_names.len() as u32;
    merged_names.extend(map.get_names().map(|n| Cow::Owned(n.to_owned())));
    offset
  };

  // Pre-compute lookup tables paired with their offsets in reverse order so we avoid reversing
  // on every token lookup.
  let mut chain_with_offsets: Vec<_> = chain_without_last
    .iter()
    .map(|sourcemap| (*sourcemap, sourcemap.generate_lookup_table(), append_names(sourcemap)))
    .collect();
  chain_with_offsets.reverse();
  let last_offset = append_names(last_map);

  // `last_offset` counts the names of every map before the last one. When it is 0, no traced
  // token can carry a name, so the remap loop skips the per-step name tracking.
  let tokens = if last_offset == 0 {
    remap_tokens::<false>(last_map, &chain_with_offsets, last_offset)
  } else {
    remap_tokens::<true>(last_map, &chain_with_offsets, last_offset)
  };

  SourceMap::new(
    None,
    merged_names,
    None,
    first_map.get_sources().map(|s| Cow::Owned(s.to_owned())).collect(),
    first_map.get_source_contents().map(|x| x.map(|s| Cow::Owned(s.to_owned()))).collect(),
    tokens,
    None,
  )
}

/// Remaps `last_map`'s tokens through `chain`, the earlier maps with the nearest one first.
/// `TRACK_NAMES` is a const so that a chain without names compiles without the name work.
fn remap_tokens<const TRACK_NAMES: bool>(
  last_map: &oxc_sourcemap::SourceMap<'_>,
  chain: &[(&oxc_sourcemap::SourceMap<'_>, Vec<&[Token]>, u32)],
  last_offset: u32,
) -> Box<[Token]> {
  last_map
    .get_source_view_tokens()
    .filter_map(|token| {
      let unmapped_token =
        || Token::new(token.get_dst_line(), token.get_dst_col(), 0, 0, None, None);
      if token.get_source_id().is_none() {
        return Some(unmapped_token());
      }

      let mut original_token = token;
      let mut name_id = token.get_name_id().map(|id| id + last_offset);
      for (sourcemap, lookup_table, offset) in chain {
        let traced = sourcemap.lookup_source_view_token_approx(
          lookup_table,
          original_token.get_src_line(),
          original_token.get_src_col(),
        )?;
        if traced.get_source_id().is_none() {
          return Some(unmapped_token());
        }
        if TRACK_NAMES {
          // Prefer the name from this (earlier) map; otherwise carry forward the downstream one.
          name_id = traced.get_name_id().map(|id| id + offset).or(name_id);
        }
        original_token = traced;
      }

      Some(Token::new(
        token.get_dst_line(),
        token.get_dst_col(),
        original_token.get_src_line(),
        original_token.get_src_col(),
        original_token.get_source_id(),
        name_id,
      ))
    })
    .collect()
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
  source_joiner.append_source(SourceMapSource::new(code, map.unwrap().into_owned()));

  let filename = "bar.js".to_string();
  let source_text = "const bar = 2; console.log(bar);\n".to_string();
  let ret2: oxc::parser::ParserReturn = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { map, code, .. } = Codegen::new()
    .with_options(CodegenOptions {
      source_map_path: Some(filename.into()),
      ..CodegenOptions::default()
    })
    .build(&ret2.program);
  source_joiner.append_source(SourceMapSource::new(code, map.unwrap().into_owned()));

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
  let map = map.unwrap().into_owned();
  sourcemap_chain.push(&map);

  let map = collapse_sourcemaps(&sourcemap_chain);
  assert_eq!(
    SourcemapVisualizer::new(&code, &map).get_text(),
    r#"- foo.js
(0:0) "const " --> (0:0) "const "
(0:6) "foo = " --> (0:6) "foo = "
(0:12) "1; " --> (0:12) "1;\n"
(0:15) "console." --> (1:0) "console."
(0:23) "log(" --> (1:8) "log("
(0:27) "foo" --> (1:12) "foo"
(0:30) ");\n" --> (1:15) ");\n"
- bar.js
(0:0) "const " --> (2:0) "const "
(0:6) "bar = " --> (2:6) "bar = "
(0:12) "2; " --> (2:12) "2;\n"
(0:15) "console." --> (3:0) "console."
(0:23) "log(" --> (3:8) "log("
(0:27) "bar" --> (3:12) "bar"
(0:30) ");\n" --> (3:15) ");\n"
"#
  );
}

#[test]
fn test_collapse_sourcemaps_clamps_before_first_token_on_a_line() {
  use oxc_sourcemap::SourceMapBuilder;

  let mut detailed_builder = SourceMapBuilder::default();
  let original_source = detailed_builder.add_source_and_content("original.js", "  target();\n");
  detailed_builder.add_token(0, 2, 0, 2, Some(original_source), None);
  let detailed_map = detailed_builder.into_sourcemap().into_owned();

  let mut coarse_builder = SourceMapBuilder::default();
  let intermediate_source = coarse_builder.add_source_and_content("intermediate.js", "target();\n");
  coarse_builder.add_token(0, 0, 0, 0, Some(intermediate_source), None);
  let coarse_map = coarse_builder.into_sourcemap().into_owned();

  let collapsed = collapse_sourcemaps(&[&detailed_map, &coarse_map]);
  let tokens = collapsed.get_tokens().collect::<Vec<_>>();

  assert_eq!(tokens.len(), 1, "a coarse token before the line's first detailed token must survive");
  assert_eq!((tokens[0].get_dst_line(), tokens[0].get_dst_col()), (0, 0));
  assert_eq!((tokens[0].get_src_line(), tokens[0].get_src_col()), (0, 2));
  assert_eq!(tokens[0].get_source_id(), Some(0));
  assert_eq!(collapsed.get_sources().collect::<Vec<_>>(), ["original.js"]);
}

#[test]
fn test_collapse_sourcemaps_preserves_an_explicitly_unmapped_final_boundary() {
  use oxc_sourcemap::SourceMapBuilder;

  let mut detailed_builder = SourceMapBuilder::default();
  let original_source = detailed_builder.add_source_and_content("original.js", "mapped unmapped\n");
  detailed_builder.add_token(0, 0, 0, 0, Some(original_source), None);
  let detailed_map = detailed_builder.into_sourcemap().into_owned();

  let mut final_builder = SourceMapBuilder::default();
  let intermediate_source =
    final_builder.add_source_and_content("intermediate.js", "mapped unmapped\n");
  final_builder.add_token(0, 0, 0, 0, Some(intermediate_source), None);
  final_builder.add_token(0, 5, 0, 0, None, None);
  let final_map = final_builder.into_sourcemap().into_owned();

  let collapsed = collapse_sourcemaps(&[&detailed_map, &final_map]);
  let lookup_table = collapsed.generate_lookup_table();

  assert_eq!(collapsed.get_tokens().count(), 2);
  assert_eq!(collapsed.lookup_token(&lookup_table, 0, 4).unwrap().get_source_id(), Some(0));
  assert_eq!(collapsed.lookup_token(&lookup_table, 0, 5).unwrap().get_source_id(), None);
  assert_eq!(collapsed.lookup_token(&lookup_table, 0, 9).unwrap().get_source_id(), None);
}

#[test]
fn test_collapse_sourcemaps_preserves_an_explicitly_unmapped_intermediate_boundary() {
  use oxc_sourcemap::SourceMapBuilder;

  let mut detailed_builder = SourceMapBuilder::default();
  let original_source = detailed_builder.add_source_and_content("original.js", "mapped unmapped\n");
  detailed_builder.add_token(0, 0, 0, 0, Some(original_source), None);
  let detailed_map = detailed_builder.into_sourcemap().into_owned();

  let mut intermediate_builder = SourceMapBuilder::default();
  let detailed_source =
    intermediate_builder.add_source_and_content("detailed.js", "mapped unmapped\n");
  intermediate_builder.add_token(0, 0, 0, 0, Some(detailed_source), None);
  intermediate_builder.add_token(0, 5, 0, 0, None, None);
  let intermediate_map = intermediate_builder.into_sourcemap().into_owned();

  let mut outer_builder = SourceMapBuilder::default();
  let intermediate_source =
    outer_builder.add_source_and_content("intermediate.js", "mapped unmapped\n");
  outer_builder.add_token(0, 0, 0, 0, Some(intermediate_source), None);
  outer_builder.add_token(0, 5, 0, 5, Some(intermediate_source), None);
  let outer_map = outer_builder.into_sourcemap().into_owned();

  let collapsed = collapse_sourcemaps(&[&detailed_map, &intermediate_map, &outer_map]);
  let lookup_table = collapsed.generate_lookup_table();

  assert_eq!(collapsed.get_tokens().count(), 2);
  assert_eq!(collapsed.lookup_token(&lookup_table, 0, 4).unwrap().get_source_id(), Some(0));
  assert_eq!(collapsed.lookup_token(&lookup_table, 0, 5).unwrap().get_source_id(), None);
  assert_eq!(collapsed.lookup_token(&lookup_table, 0, 9).unwrap().get_source_id(), None);
}

/// Test for https://github.com/rollup/rollup/issues/5955
#[test]
fn test_collapse_sourcemaps_with_coarse_segments() {
  use oxc_sourcemap::SourceMap;

  fn get_loc(mut pos: usize, code: &str) -> (u32, u32) {
    for (line_idx, line) in code.lines().enumerate() {
      if pos <= line.len() {
        #[expect(clippy::cast_possible_truncation)]
        return (line_idx as u32, pos as u32);
      }
      pos -= line.len() + 1; // +1 for newline
    }
    panic!("position out of bounds");
  }

  let original_code = "import { useEffect } from 'react';

export function App() {
  useEffect(() => {
    console.log('ReplayAnalyze');
  }, []);

  return <div>{'.'}</div>;
}
";
  let transformed_code = r#"import{jsx}from"react/jsx-runtime";import{useEffect}from"react";export function App(){return useEffect((()=>{console.log("ReplayAnalyze")}),[]),jsx("div",{children:"."})}"#;

  // spellchecker:off
  let esbuild_map_json = r#"{
    "version": 3,
    "sources": ["<stdin>"],
    "sourcesContent": ["import { useEffect } from 'react';\n\nexport function App() {\n  useEffect(() => {\n    console.log('ReplayAnalyze');\n  }, []);\n\n  return <div>{'.'}</div>;\n}\n"],
    "mappings": "AAOS;AAPT,SAAS,iBAAiB;AAEnB,gBAAS,MAAM;AACpB,YAAU,MAAM;AACd,YAAQ,IAAI,eAAe;AAAA,EAC7B,GAAG,CAAC,CAAC;AAEL,SAAO,oBAAC,SAAK,eAAI;AACnB;",
    "names": []
  }"#;
  // spellchecker:on
  let esbuild_map = SourceMap::from_json_string(esbuild_map_json).unwrap();

  // spellchecker:off
  let terser_map_json = r#"{
    "version": 3,
    "names": ["jsx", "useEffect", "App", "console", "log", "children"],
    "sources": ["0"],
    "sourcesContent": ["import { jsx } from \"react/jsx-runtime\";\nimport { useEffect } from \"react\";\nexport function App() {\n  useEffect(() => {\n    console.log(\"ReplayAnalyze\");\n  }, []);\n  return /* @__PURE__ */ jsx(\"div\", { children: \".\" });\n}\n"],
    "mappings": "OAASA,QAAW,2BACXC,cAAiB,eACnB,SAASC,MAId,OAHAD,WAAU,KACRE,QAAQC,IAAI,gBAAgB,GAC3B,IACoBJ,IAAI,MAAO,CAAEK,SAAU,KAChD"
  }"#;
  // spellchecker:on
  let terser_map = SourceMap::from_json_string(terser_map_json).unwrap();

  let collapsed = collapse_sourcemaps(&[&esbuild_map, &terser_map]);
  let collapsed_lookup_table = collapsed.generate_lookup_table();

  let generated_loc = get_loc(transformed_code.find("return").unwrap(), transformed_code);
  let original_loc = collapsed
    .lookup_source_view_token(&collapsed_lookup_table, generated_loc.0, generated_loc.1)
    .map(|token| (token.get_src_line(), token.get_src_col()));
  assert_eq!(
    original_loc,
    Some(get_loc(original_code.find("return").unwrap(), original_code)),
    "collapsed sourcemap should map 'return' in transformed code back to original source"
  );
}

/// Names introduced only by the last map in the chain must survive collapsing.
#[test]
fn test_collapse_sourcemaps_preserves_last_map_names() {
  use oxc_sourcemap::SourceMapBuilder;

  let mut first_builder = SourceMapBuilder::default();
  let first_source_id =
    first_builder.set_source_and_content("source.ts", "function topLevelDemo() {}");
  first_builder.add_token(0, 0, 0, 0, Some(first_source_id), None);
  first_builder.add_token(0, 9, 0, 9, Some(first_source_id), None);
  let first_map = first_builder.into_sourcemap();

  let mut last_builder = SourceMapBuilder::default();
  let last_source_id =
    last_builder.set_source_and_content("bundle.js", "function topLevelDemo() {}");
  let top_level_demo = last_builder.add_name("topLevelDemo");
  last_builder.add_token(0, 0, 0, 0, Some(last_source_id), None);
  last_builder.add_token(0, 9, 0, 9, Some(last_source_id), Some(top_level_demo));
  let last_map = last_builder.into_sourcemap();

  let collapsed = collapse_sourcemaps(&[&first_map, &last_map]);
  let lookup = collapsed.generate_lookup_table();
  let token =
    collapsed.lookup_source_view_token(&lookup, 0, 9).expect("token at generated position (0:9)");
  assert_eq!(token.get_name().map(AsRef::as_ref), Some("topLevelDemo"));
}

/// In a 3-map chain, a name from a middle map (e.g. an intermediate transform) must survive
/// when neither the first nor the last map names the corresponding position.
#[test]
fn test_collapse_sourcemaps_preserves_middle_map_names() {
  use oxc_sourcemap::SourceMapBuilder;

  let mut first_builder = SourceMapBuilder::default();
  let first_source_id = first_builder.set_source_and_content("source.ts", "const foo = 1");
  first_builder.add_token(0, 6, 0, 6, Some(first_source_id), None);
  let first_map = first_builder.into_sourcemap();

  let mut middle_builder = SourceMapBuilder::default();
  let middle_source_id = middle_builder.set_source_and_content("first.js", "const foo = 1");
  let foo = middle_builder.add_name("foo");
  middle_builder.add_token(0, 6, 0, 6, Some(middle_source_id), Some(foo));
  let middle_map = middle_builder.into_sourcemap();

  let mut last_builder = SourceMapBuilder::default();
  let last_source_id = last_builder.set_source_and_content("middle.js", "const foo = 1");
  last_builder.add_token(0, 6, 0, 6, Some(last_source_id), None);
  let last_map = last_builder.into_sourcemap();

  let collapsed = collapse_sourcemaps(&[&first_map, &middle_map, &last_map]);
  let lookup = collapsed.generate_lookup_table();
  let token =
    collapsed.lookup_source_view_token(&lookup, 0, 6).expect("token at generated position (0:6)");
  assert_eq!(token.get_name().map(AsRef::as_ref), Some("foo"));
}

/// When both maps name a position, prefer the first map's (the earlier original).
#[test]
fn test_collapse_sourcemaps_prefers_first_map_name() {
  use oxc_sourcemap::SourceMapBuilder;

  let mut first_builder = SourceMapBuilder::default();
  let first_source_id =
    first_builder.set_source_and_content("source.ts", "const DEBUG_BUILD = true");
  let debug_build = first_builder.add_name("DEBUG_BUILD");
  first_builder.add_token(0, 6, 0, 6, Some(first_source_id), Some(debug_build));
  let first_map = first_builder.into_sourcemap();

  let mut last_builder = SourceMapBuilder::default();
  let last_source_id =
    last_builder.set_source_and_content("bundle.js", "const DEBUG_BUILD$1 = true");
  let intermediate = last_builder.add_name("DEBUG_BUILD$1");
  last_builder.add_token(0, 6, 0, 6, Some(last_source_id), Some(intermediate));
  let last_map = last_builder.into_sourcemap();

  let collapsed = collapse_sourcemaps(&[&first_map, &last_map]);
  let lookup = collapsed.generate_lookup_table();
  let token =
    collapsed.lookup_source_view_token(&lookup, 0, 6).expect("token at generated position (0:6)");
  assert_eq!(token.get_name().map(AsRef::as_ref), Some("DEBUG_BUILD"));
}
