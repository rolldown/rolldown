use std::path::PathBuf;

use arcstr::ArcStr;
use oxc::{
  allocator::Allocator,
  codegen::{CodeGenerator, Codegen, CodegenOptions, CodegenReturn},
  minifier::{Minifier, MinifierOptions},
  parser::{ParseOptions, Parser},
  sourcemap::SourceMap,
  span::SourceType,
};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};

use crate::ecma_ast::{
  program_cell::{ProgramCell, ProgramCellDependent, ProgramCellOwner},
  EcmaAst,
};
pub struct EcmaCompiler;

impl EcmaCompiler {
  pub fn parse(
    filename: &str,
    source: impl Into<ArcStr>,
    ty: SourceType,
  ) -> DiagnosableResult<EcmaAst> {
    let source: ArcStr = source.into();
    let allocator = oxc::allocator::Allocator::default();
    let inner =
      ProgramCell::try_new(ProgramCellOwner { source: source.clone(), allocator }, |owner| {
        let parser = Parser::new(&owner.allocator, &owner.source, ty).with_options(ParseOptions {
          allow_return_outside_function: true,
          ..ParseOptions::default()
        });
        let ret = parser.parse();
        if ret.panicked || !ret.errors.is_empty() {
          Err(
            ret
              .errors
              .into_iter()
              .map(|mut error| {
                let error = &mut *error;
                BuildDiagnostic::oxc_parse_error(
                  source.clone(),
                  filename.to_string(),
                  error.help.take().unwrap_or_default().into(),
                  error.message.to_string(),
                  error.labels.take().unwrap_or_default(),
                )
              })
              .collect::<Vec<_>>(),
          )
        } else {
          Ok(ProgramCellDependent { program: ret.program })
        }
      })?;
    Ok(EcmaAst { program: inner, source_type: ty, contains_use_strict: false })
  }

  pub fn print(ast: &EcmaAst, filename: &str, enable_source_map: bool) -> CodegenReturn {
    CodeGenerator::new()
      .with_options(CodegenOptions {
        comments: true,
        source_map_path: enable_source_map.then(|| PathBuf::from(filename)),
        ..CodegenOptions::default()
      })
      .build(ast.program())
  }

  pub fn minify(
    source_text: &str,
    enable_sourcemap: bool,
    filename: &str,
  ) -> anyhow::Result<(String, Option<SourceMap>)> {
    let allocator = Allocator::default();
    let program = Parser::new(&allocator, source_text, SourceType::default()).parse().program;
    let program = allocator.alloc(program);
    let options = MinifierOptions { mangle: true, ..MinifierOptions::default() };
    let ret = Minifier::new(options).build(&allocator, program);
    let ret = Codegen::new()
      .with_options(CodegenOptions {
        source_map_path: enable_sourcemap.then(|| PathBuf::from(filename)),
        minify: true,
        ..CodegenOptions::default()
      })
      .with_mangler(ret.mangler)
      .build(program);
    Ok((ret.code, ret.map))
  }
}

#[test]
fn basic_test() {
  let ast = EcmaCompiler::parse("", "const a = 1;".to_string(), SourceType::default()).unwrap();
  let code = EcmaCompiler::print(&ast, "", false).code;
  assert_eq!(code, "const a = 1;\n");
}
