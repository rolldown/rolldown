use oxc::{
  allocator::Allocator,
  parser::{ParseOptions, Parser, ParserReturn},
  span::SourceType,
};
use rolldown_common::ModuleType;

pub fn parse_program<'a>(
  allocator: &'a Allocator,
  code: &'a str,
  module_type: &ModuleType,
  id: &str,
) -> anyhow::Result<Option<ParserReturn<'a>>> {
  if !matches!(module_type, ModuleType::Js | ModuleType::Ts | ModuleType::Jsx | ModuleType::Tsx) {
    return Ok(None);
  }

  let source_type = match module_type {
    ModuleType::Js => SourceType::mjs(),
    ModuleType::Jsx => SourceType::jsx(),
    ModuleType::Ts => SourceType::ts(),
    ModuleType::Tsx => SourceType::tsx(),
    _ => unreachable!(),
  };
  let parser_ret = Parser::new(allocator, code, source_type)
    .with_options(ParseOptions { preserve_parens: false, ..ParseOptions::default() })
    .parse();

  if parser_ret.panicked
    && let Some(err) =
      parser_ret.diagnostics.iter().find(|e| e.severity == oxc::diagnostics::Severity::Error)
  {
    return Err(anyhow::anyhow!(format!("Failed to parse code in '{}': {:?}", id, err.message)));
  }

  Ok(Some(parser_ret))
}
