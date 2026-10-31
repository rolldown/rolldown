use oxc_sourcemap::ConcatSourceMapBuilder;

use crate::SourceMap;
use crate::source::Source;

#[derive(Default)]
pub struct SourceJoiner<'source> {
  inner: Vec<Box<dyn Source + Send + 'source>>,
  prepend_source: Vec<Box<dyn Source + Send + 'source>>,
  pub enable_sourcemap: bool,
  names_len: usize,
  sources_len: usize,
  tokens_len: usize,
  token_chunks_len: usize,
}

impl<'source> SourceJoiner<'source> {
  pub fn append_source<T: Source + Send + 'source>(&mut self, source: T) {
    if let Some(sourcemap) = source.sourcemap() {
      self.accumulate_sourcemap_data_size(sourcemap);
    }
    self.inner.push(Box::new(source));
  }

  pub fn append_source_dyn(&mut self, source: Box<dyn Source + Send + 'source>) {
    if let Some(sourcemap) = source.sourcemap() {
      self.accumulate_sourcemap_data_size(sourcemap);
    }
    self.inner.push(source);
  }

  pub fn prepend_source<T: Source + Send + 'source>(&mut self, source: T) {
    if let Some(sourcemap) = source.sourcemap() {
      self.accumulate_sourcemap_data_size(sourcemap);
    }
    self.prepend_source.push(Box::new(source));
  }

  pub fn join(&mut self) -> (String, Option<SourceMap>) {
    let sources_len = self.prepend_source.len() + self.inner.len();

    let size_hint_of_ret_source = self
      .prepend_source
      .iter()
      .chain(self.inner.iter())
      .map(|source| source.content().len())
      .sum::<usize>()
      + sources_len;
    let mut ret_source = String::with_capacity(size_hint_of_ret_source);

    let mut sourcemap_builder = self.enable_sourcemap.then(|| {
      ConcatSourceMapBuilder::with_capacity(
        self.names_len,
        self.sources_len,
        self.tokens_len,
        self.token_chunks_len,
      )
    });
    if let Some(sourcemap_builder) = &mut sourcemap_builder {
      let mut line_offset = 0;
      // Move exclusively owned maps into the builder. A caller may still pass
      // a shared source by reference; keep borrowing that map so its mappings
      // are preserved, then copy only its borrowed strings when detaching.
      for (index, source) in self.prepend_source.iter_mut().chain(self.inner.iter_mut()).enumerate()
      {
        ret_source.push_str(source.content());
        if let Some(map) = source.take_sourcemap() {
          sourcemap_builder.add_sourcemap_owned(map, line_offset);
        } else if let Some(map) = source.sourcemap() {
          sourcemap_builder.add_sourcemap(map, line_offset);
        }
        // The line count only advances the offset for a *following* source, so
        // scan it inside this branch — never for the final source, which can be
        // a whole chunk appended without a pre-computed count.
        if index < sources_len - 1 {
          ret_source.push('\n');
          line_offset += source.lines_count() + 1; // +1 for the newline
        }
      }
    } else {
      // Without a sourcemap there is no line offset to maintain. Avoid scanning
      // every source for newlines on this common path.
      for (index, source) in self.prepend_source.iter().chain(self.inner.iter()).enumerate() {
        ret_source.push_str(source.content());
        if index < sources_len - 1 {
          ret_source.push('\n');
        }
      }
    }
    let map = sourcemap_builder.map(|builder| {
      let mut parts = builder.into_owned_sourcemap().into_inner().into_parts();
      // oxc_sourcemap 9's concat builder takes original-coordinate baselines from the last raw
      // token. Unmapped tokens do not update those fields in the VLQ stream, so carry the last
      // mapped coordinates across chunk boundaries instead. Keep the token/chunk allocations.
      // See internal-docs/chunk-ast-minify/implementation.md.
      let mut previous = (0, 0);
      if let Some(chunks) = &mut parts.token_chunks {
        for chunk in chunks {
          chunk.prev_src_line = previous.0;
          chunk.prev_src_col = previous.1;
          if let Some(token) = parts.tokens[chunk.start as usize..chunk.end as usize]
            .iter()
            .rev()
            .find(|token| token.get_source_id().is_some())
          {
            previous = (token.get_src_line(), token.get_src_col());
          }
        }
      }
      SourceMap::from(parts)
    });
    (ret_source, map)
  }

  fn accumulate_sourcemap_data_size(&mut self, hint: &SourceMap) {
    self.enable_sourcemap = true;
    self.names_len += hint.get_names().count();
    self.sources_len += hint.get_sources().count();
    self.tokens_len += hint.get_tokens().count();
    self.token_chunks_len += 1;
  }
}

#[test]
fn test_concat_sourcemaps() {
  use crate::{SourceJoiner, SourceMapSource};
  use oxc::{
    allocator::Allocator,
    codegen::{Codegen, CodegenOptions, CodegenReturn, CommentOptions},
    parser::Parser,
    span::SourceType,
  };
  use oxc_sourcemap::SourcemapVisualizer;

  let mut source_joiner = SourceJoiner::default();
  source_joiner.append_source("\nconsole.log()".to_string());
  source_joiner.prepend_source("// banner".to_string());

  let filename = "foo.js".to_string();
  let allocator = Allocator::default();
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

  let (content, map) = source_joiner.join();

  assert_eq!(
    &content,
    r"// banner

console.log()
const foo = 1;
console.log(foo);
"
  );
  assert_eq!(
    SourcemapVisualizer::new(&content, &map.unwrap()).get_text(),
    r#"- foo.js
(0:0) "const " --> (3:0) "const "
(0:6) "foo = " --> (3:6) "foo = "
(0:12) "1; " --> (3:12) "1;\n"
(0:15) "console." --> (4:0) "console."
(0:23) "log(" --> (4:8) "log("
(0:27) "foo" --> (4:12) "foo"
(0:30) ");\n" --> (4:15) ");\n"
"#
  );
}

#[test]
fn test_concat_serialization_after_unmapped_boundaries() {
  use oxc_sourcemap::{SourceMapParts, Token};

  use crate::SourceMapSource;

  for (end_unmapped, glue_count) in [(false, 0), (true, 0), (false, 2), (true, 2)] {
    let map = |tokens: Vec<Token>| {
      SourceMap::from(SourceMapParts {
        sources: vec!["shared.js".into()],
        source_contents: vec![Some("\n\n   first\n     second\n     third".into())],
        names: vec!["unused".into(), "first".into()],
        tokens: tokens.into_boxed_slice(),
        ..Default::default()
      })
    };
    let mut first =
      vec![Token::new(0, 0, 2, 3, Some(0), Some(1)), Token::new(0, 4, 3, 5, Some(0), None)];
    if end_unmapped {
      first.push(Token::new(0, 8, 0, 0, None, None));
    }
    let mut joiner = SourceJoiner::default();
    joiner.append_source(SourceMapSource::new("first();".to_string(), map(first)));
    // Consecutive entirely unmapped fragments must carry the last mapped coordinates and name.
    for _ in 0..glue_count {
      joiner.append_source(SourceMapSource::new(
        "glue();".to_string(),
        map(vec![Token::new(0, 0, 0, 0, None, None)]),
      ));
    }
    joiner.append_source(SourceMapSource::new(
      "last();".to_string(),
      map(vec![Token::new(0, 0, 2, 3, Some(0), Some(1)), Token::new(0, 4, 4, 5, Some(0), None)]),
    ));
    let (_, map) = joiner.join();
    let map = map.unwrap();
    let json = map.to_json_string();
    let decoded = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
    let coordinates = |token: Token| {
      (
        token.get_dst_line(),
        token.get_dst_col(),
        token.get_source_id().map(|source| (source, token.get_src_line(), token.get_src_col())),
        token.get_name_id(),
      )
    };
    assert_eq!(
      decoded.get_tokens().map(coordinates).collect::<Vec<_>>(),
      map.get_tokens().map(coordinates).collect::<Vec<_>>(),
      "serialization must preserve mapped coordinates ({end_unmapped}, {glue_count})"
    );
    assert_eq!(decoded.get_sources().collect::<Vec<_>>(), map.get_sources().collect::<Vec<_>>());
    assert_eq!(decoded.get_names().collect::<Vec<_>>(), map.get_names().collect::<Vec<_>>());
  }
}

#[test]
fn test_concat_mixed_owned_and_borrowed_sourcemaps() {
  use std::borrow::Cow;

  use crate::{Source, SourceJoiner, SourceMap, SourceMapSource};
  use oxc_sourcemap::Token;

  fn map(filename: &str, source_content: &str) -> SourceMap {
    SourceMap::new(
      None,
      vec![],
      None,
      vec![Cow::Owned(filename.to_string())],
      vec![Some(Cow::Owned(source_content.to_string()))],
      vec![Token::new(0, 0, 0, 0, Some(0), None)].into_boxed_slice(),
      None,
    )
  }

  // Callers may still pass a shared source that cannot yield ownership of its map.
  let borrowed_source: Box<dyn Source + Send + Sync> = Box::new(SourceMapSource::new(
    "borrowed();".to_string(),
    map("borrowed.js", "borrowed source"),
  ));

  let mut joiner = SourceJoiner::default();
  joiner.append_source(&borrowed_source);
  joiner
    .append_source(SourceMapSource::new("owned();".to_string(), map("owned.js", "owned source")));

  let (content, map) = joiner.join();
  let map = map.expect("both input sourcemaps should be preserved");

  assert_eq!(content, "borrowed();\nowned();");
  assert_eq!(map.get_sources().collect::<Vec<_>>(), ["borrowed.js", "owned.js"]);
  assert_eq!(map.get_source_content(0), Some("borrowed source"));
  assert_eq!(map.get_source_content(1), Some("owned source"));
  assert_eq!(map.get_token(0).and_then(|token| token.get_source_id()), Some(0));
  assert_eq!(map.get_token(1).map(|token| token.get_dst_line()), Some(1));
  assert_eq!(map.get_token(1).and_then(|token| token.get_source_id()), Some(1));
}
