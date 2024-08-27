use oxc::sourcemap::{ConcatSourceMapBuilder, SourceMap};

use crate::lines_count;

pub trait Source {
  fn sourcemap(&self) -> Option<&SourceMap>;
  fn content(&self) -> &String;
  fn lines_count(&self) -> u32;
  #[allow(clippy::wrong_self_convention)]
  fn into_concat_source(
    &self,
    final_source: &mut String,
    sourcemap_builder: &mut Option<ConcatSourceMapBuilder>,
    line_offset: u32,
  );
}

pub struct RawSource {
  content: String,
}

impl RawSource {
  pub fn new(content: String) -> Self {
    Self { content }
  }
}

impl Source for RawSource {
  fn sourcemap(&self) -> Option<&SourceMap> {
    None
  }

  fn content(&self) -> &String {
    &self.content
  }

  fn lines_count(&self) -> u32 {
    lines_count(&self.content)
  }

  fn into_concat_source(
    &self,
    final_source: &mut String,
    _sourcemap_builder: &mut Option<ConcatSourceMapBuilder>,
    _line_offset: u32,
  ) {
    final_source.push_str(&self.content);
  }
}

pub struct SourceMapSource {
  content: String,
  sourcemap: SourceMap,
  lines_count: u32,
}

impl SourceMapSource {
  pub fn new(content: String, sourcemap: SourceMap, lines_count: u32) -> Self {
    Self { content, sourcemap, lines_count }
  }
}

impl Source for SourceMapSource {
  fn sourcemap(&self) -> Option<&SourceMap> {
    Some(&self.sourcemap)
  }

  fn content(&self) -> &String {
    &self.content
  }

  fn lines_count(&self) -> u32 {
    self.lines_count
  }

  fn into_concat_source(
    &self,
    final_source: &mut String,
    sourcemap_builder: &mut Option<ConcatSourceMapBuilder>,
    line_offset: u32,
  ) {
    if let Some(sourcemap_builder) = sourcemap_builder {
      sourcemap_builder.add_sourcemap(&self.sourcemap, line_offset);
    }

    final_source.push_str(&self.content);
  }
}

#[derive(Default)]
pub struct ConcatSource {
  inner: Vec<Box<dyn Source + Send>>,
  prepend_source: Vec<Box<dyn Source + Send>>,
  enable_sourcemap: bool,
  names_len: usize,
  sources_len: usize,
  tokens_len: usize,
  token_chunks_len: usize,
}

impl ConcatSource {
  fn add_sourcemap(&mut self, sourcemap: &SourceMap) {
    self.enable_sourcemap = true;
    self.names_len += sourcemap.get_names().count();
    self.sources_len += sourcemap.get_sources().count();
    self.tokens_len += sourcemap.get_tokens().count();
    self.token_chunks_len += 1;
  }

  pub fn add_source(&mut self, source: Box<dyn Source + Send>) {
    if let Some(sourcemap) = source.sourcemap() {
      self.add_sourcemap(sourcemap);
    }
    self.inner.push(source);
  }

  pub fn add_prepend_source(&mut self, source: Box<dyn Source + Send>) {
    if let Some(sourcemap) = source.sourcemap() {
      self.add_sourcemap(sourcemap);
    }
    self.prepend_source.push(source);
  }

  pub fn content_and_sourcemap(self) -> (String, Option<SourceMap>) {
    let mut final_source = String::new();
    let mut sourcemap_builder = self.enable_sourcemap.then(|| {
      ConcatSourceMapBuilder::with_capacity(
        self.names_len,
        self.sources_len,
        self.tokens_len,
        self.token_chunks_len,
      )
    });
    let mut line_offset = 0;
    let source_len = self.prepend_source.len() + self.inner.len();

    for (index, source) in self.prepend_source.iter().chain(self.inner.iter()).enumerate() {
      source.into_concat_source(&mut final_source, &mut sourcemap_builder, line_offset);
      if index < source_len - 1 {
        final_source.push('\n');
        line_offset += source.lines_count() + 1; // +1 for the newline
      }
    }

    (final_source, sourcemap_builder.map(ConcatSourceMapBuilder::into_sourcemap))
  }
}

#[test]
fn test_concat_sourcemaps() {
  use crate::{ConcatSource, RawSource, SourceMapSource};
  use oxc::{
    allocator::Allocator,
    codegen::{CodeGenerator, CodegenReturn},
    parser::Parser,
    sourcemap::SourcemapVisualizer,
    span::SourceType,
  };

  let mut concat_source = ConcatSource::default();
  concat_source.add_source(Box::new(RawSource::new("\nconsole.log()".to_string())));
  concat_source.add_prepend_source(Box::new(RawSource::new("// banner".to_string())));

  let filename = "foo.js".to_string();
  let allocator = Allocator::default();
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

  let (content, map) = concat_source.content_and_sourcemap();

  assert_eq!(content, "// banner\n\nconsole.log()\nconst foo = 1;\nconsole.log(foo);\n");

  assert_eq!(
    SourcemapVisualizer::new(&content, &map.unwrap()).into_visualizer_text(),
    r#"- foo.js
(0:0-0:6) "const " --> (3:0-3:6) "\nconst"
(0:6-0:12) "foo = " --> (3:6-3:12) " foo ="
(0:12-0:15) "1; " --> (3:12-4:0) " 1;"
(0:15-0:23) "console." --> (4:0-4:8) "\nconsole"
(0:23-0:27) "log(" --> (4:8-4:12) ".log"
(0:27-0:31) "foo)" --> (4:12-4:16) "(foo"
(0:31-1:1) ";\n" --> (4:16-5:1) ");\n"
"#
  );
}
