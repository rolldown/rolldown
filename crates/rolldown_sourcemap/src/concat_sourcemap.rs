// cSpell:disable
use oxc::sourcemap::{ConcatSourceMapBuilder, SourceMap};

pub trait Source {
  fn sourcemap(&self) -> Option<&SourceMap>;
  fn content(&self) -> &String;
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
}

impl SourceMapSource {
  pub fn new(content: String, sourcemap: SourceMap) -> Self {
    Self { content, sourcemap }
  }
}

impl Source for SourceMapSource {
  fn sourcemap(&self) -> Option<&SourceMap> {
    Some(&self.sourcemap)
  }

  fn content(&self) -> &String {
    &self.content
  }

  #[allow(clippy::cast_possible_truncation)]
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
}

impl ConcatSource {
  pub fn add_source(&mut self, source: Box<dyn Source + Send>) {
    if source.sourcemap().is_some() {
      self.enable_sourcemap = true;
    }
    self.inner.push(source);
  }

  pub fn add_prepend_source(&mut self, source: Box<dyn Source + Send>) {
    if source.sourcemap().is_some() {
      self.enable_sourcemap = true;
    }
    self.prepend_source.push(source);
  }

  #[allow(clippy::cast_possible_truncation)]
  pub fn content_and_sourcemap(self) -> (String, Option<SourceMap>) {
    let mut final_source = String::new();
    let mut sourcemap_builder = self.enable_sourcemap.then_some(ConcatSourceMapBuilder::default());
    let mut line_offset = 0;
    let source_len = self.prepend_source.len() + self.inner.len();

    for (index, source) in self.prepend_source.iter().chain(self.inner.iter()).enumerate() {
      source.into_concat_source(&mut final_source, &mut sourcemap_builder, line_offset);
      if index < source_len - 1 {
        final_source.push('\n');
        line_offset += source.content().matches('\n').count() as u32 + 1; // +1 for the newline
      }
    }

    (final_source, sourcemap_builder.map(ConcatSourceMapBuilder::into_sourcemap))
  }
}

#[cfg(test)]
mod tests {
  pub use crate::SourceMap;

  use crate::{ConcatSource, RawSource, SourceMapSource};
  #[test]
  fn concat_sourcemaps_works() {
    let mut concat_source = ConcatSource::default();
    concat_source.add_source(Box::new(RawSource::new("\nconsole.log()".to_string())));
    concat_source.add_prepend_source(Box::new(RawSource::new("// banner".to_string())));

    concat_source.add_source(Box::new(SourceMapSource::new(
      "function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n".to_string(),
      SourceMap::from_json_string(
        r#"{
          "version":3,
          "sourceRoot":"",
          "mappings":"AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC",
          "sources":["index.ts"],
          "sourcesContent":["function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"],
          "names":[]
        }"#
    )
    .unwrap(),
    )));

    let (content, map) = {
      let (content, map) = concat_source.content_and_sourcemap();
      (content, map.expect("should have sourcemap").to_json_string())
    };

    assert_eq!(
      content,
      "// banner\n\nconsole.log()\nfunction sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"
    );
    let expected = "{\"version\":3,\"names\":[],\"sources\":[\"index.ts\"],\"sourcesContent\":[\"function sayHello(name: string) {\\n  console.log(`Hello, ${name}`);\\n}\\n\"],\"mappings\":\";;;AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC\"}";
    assert_eq!(map, expected);
  }
}
