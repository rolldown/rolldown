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

  /// Concatenate the pushed sources into one string + combined sourcemap.
  ///
  /// Consumes `self` so the owned input sourcemaps can be **moved** into the concat builder
  /// (`add_sourcemap_owned`) rather than borrowed and recopied — no name/source/sourcesContent
  /// string is copied. For rolldown's `'static` maps, `into_sourcemap` then returns the combined
  /// `'static` map by moving the vectors out.
  pub fn join(self) -> (String, Option<SourceMap>) {
    let Self {
      inner,
      prepend_source,
      enable_sourcemap,
      names_len,
      sources_len,
      tokens_len,
      token_chunks_len,
    } = self;

    let source_count = prepend_source.len() + inner.len();

    let size_hint_of_ret_source =
      prepend_source.iter().chain(inner.iter()).map(|source| source.content().len()).sum::<usize>()
        + source_count;
    let mut ret_source = String::with_capacity(size_hint_of_ret_source);

    let mut line_offset = 0;

    let mut sourcemap_builder: Option<ConcatSourceMapBuilder<'static>> =
      enable_sourcemap.then(|| {
        ConcatSourceMapBuilder::with_capacity(names_len, sources_len, tokens_len, token_chunks_len)
      });
    // Consume the sources, *moving* each owned sourcemap into the builder — no string is copied.
    for (index, mut source) in prepend_source.into_iter().chain(inner).enumerate() {
      if let Some(sourcemap_builder) = &mut sourcemap_builder {
        if let Some(map) = source.take_sourcemap() {
          sourcemap_builder.add_sourcemap_owned(map, line_offset);
        }
      }
      ret_source.push_str(source.content());
      if index < source_count - 1 {
        ret_source.push('\n');
        line_offset += source.lines_count() + 1; // +1 for the newline
      }
    }
    // `into_sourcemap` moves the accumulated `'static` entries out — no copy.
    (ret_source, sourcemap_builder.map(ConcatSourceMapBuilder::into_sourcemap))
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
