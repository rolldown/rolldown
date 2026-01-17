use crate::types::Result;
use anyhow::Context;
use oxc::allocator::Allocator;
use oxc::parser::{Parser, ParserReturn};
use oxc::span::SourceType;

pub struct TypeScriptParser<'a> {
  allocator: &'a Allocator,
}

impl<'a> TypeScriptParser<'a> {
  pub fn new(allocator: &'a Allocator) -> Self {
    Self { allocator }
  }

  pub fn parse(&self, source: &'a str, filename: &str) -> Result<ParserReturn<'a>> {
    let source_type = SourceType::from_path(filename)
      .with_context(|| format!("Invalid source type for file: {filename}"))?
      .with_typescript(true)
      .with_typescript_definition(true);

    let parser_return = Parser::new(self.allocator, source, source_type).parse();

    if !parser_return.errors.is_empty() {
      let errors: Vec<String> = parser_return.errors.iter().map(|e| format!("{e:?}")).collect();
      anyhow::bail!("Parse errors: {}", errors.join(", "));
    }

    Ok(parser_return)
  }
}
