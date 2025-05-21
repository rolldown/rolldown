use oxc_sourcemap::{ConcatSourceMapBuilder, SourceMap};

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

  pub fn prepend_source<T: Source + Send + 'source>(&mut self, source: T) {
    if let Some(sourcemap) = source.sourcemap() {
      self.accumulate_sourcemap_data_size(sourcemap);
    }
    self.prepend_source.push(Box::new(source));
  }

  pub fn join(&self) -> (String, Option<SourceMap>) {
    let sources_len = self.prepend_source.len() + self.inner.len();
    let sources_iter = self.prepend_source.iter().chain(self.inner.iter()).enumerate();

    let size_hint_of_ret_source =
      sources_iter.clone().map(|(_idx, source)| source.content().len()).sum::<usize>()
        + sources_len;
    let mut ret_source = String::with_capacity(size_hint_of_ret_source);

    let mut line_offset = 0;

    let mut sourcemap_builder = self.enable_sourcemap.then(|| {
      ConcatSourceMapBuilder::with_capacity(
        self.names_len,
        self.sources_len,
        self.tokens_len,
        self.token_chunks_len,
      )
    });
    for (index, source) in sources_iter {
      if let Some(sourcemap_builder) = &mut sourcemap_builder {
        source.sourcemap().inspect(|map| {
          sourcemap_builder.add_sourcemap(map, line_offset);
        });
      }
      ret_source.push_str(source.content());
      if index < sources_len - 1 {
        ret_source.push('\n');
        line_offset += source.lines_count() + 1; // +1 for the newline
      }
    }
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
    codegen::{Codegen, CodegenOptions, CodegenReturn},
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
      comments: false,
      source_map_path: Some(filename.into()),
      ..CodegenOptions::default()
    })
    .build(&ret1.program);
  source_joiner.append_source(SourceMapSource::new(code, map.as_ref().unwrap().clone()));

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
    SourcemapVisualizer::new(&content, &map.unwrap()).into_visualizer_text(),
    r#"- foo.js
(0:0) "const " --> (3:0) "const "
(0:6) "foo = " --> (3:6) "foo = "
(0:12) "1; " --> (3:12) "1;\n"
(0:15) "console." --> (4:0) "console."
(0:23) "log(" --> (4:8) "log("
(0:27) "foo)" --> (4:12) "foo)"
(0:31) ";\n" --> (4:16) ";\n"
"#
  );
}
