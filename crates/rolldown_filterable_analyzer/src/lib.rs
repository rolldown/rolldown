use oxc::allocator::Allocator;
use oxc::parser::{ParseOptions, Parser, ParserReturn};
use oxc::span::SourceType;
use rolldown_error::{BuildDiagnostic, DiagnosableResult};

pub fn parse<'a>(
  filename: &str,
  source: &'a str,
  alloc: &'a Allocator,
) -> DiagnosableResult<ParserReturn<'a>> {
  let parser = Parser::new(alloc, source, SourceType::default())
    .with_options(ParseOptions { allow_return_outside_function: true, ..ParseOptions::default() });
  let ret = parser.parse();
  if ret.panicked || !ret.errors.is_empty() {
    Err(
      ret
        .errors
        .iter()
        .map(|error| {
          BuildDiagnostic::oxc_parse_error(
            source.into(),
            filename.to_string(),
            error.help.clone().unwrap_or_default().into(),
            error.message.to_string(),
            error.labels.clone().unwrap_or_default(),
          )
        })
        .collect::<Vec<_>>(),
    )
  } else {
    Ok(ret)
  }
}

