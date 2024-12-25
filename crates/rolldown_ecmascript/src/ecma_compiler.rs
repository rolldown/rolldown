use std::path::PathBuf;

use arcstr::ArcStr;
use either::Either;
use oxc::{
  allocator::Allocator,
  ast::AstBuilder,
  codegen::{CodeGenerator, Codegen, CodegenOptions, CodegenReturn, LegalComment},
  minifier::{Minifier, MinifierOptions},
  parser::{ParseOptions, Parser},
  span::{SourceType, SPAN},
};
use oxc_sourcemap::SourceMap;
use rolldown_error::{BuildDiagnostic, BuildResult, Severity};

use crate::ecma_ast::{
  program_cell::{ProgramCell, ProgramCellDependent, ProgramCellOwner},
  EcmaAst,
};
pub struct EcmaCompiler;

impl EcmaCompiler {
  pub fn parse(filename: &str, source: impl Into<ArcStr>, ty: SourceType) -> BuildResult<EcmaAst> {
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
          Err(BuildDiagnostic::from_oxc_diagnostics(
            ret.errors,
            &source.clone(),
            filename,
            &Severity::Error,
          ))
        } else {
          Ok(ProgramCellDependent { program: ret.program })
        }
      })?;
    Ok(EcmaAst { program: inner, source_type: ty, contains_use_strict: false })
  }

  pub fn parse_expr_as_program(
    filename: &str,
    source: impl Into<ArcStr>,
    ty: SourceType,
  ) -> BuildResult<EcmaAst> {
    let source: ArcStr = source.into();
    let allocator = oxc::allocator::Allocator::default();
    let inner =
      ProgramCell::try_new(ProgramCellOwner { source: source.clone(), allocator }, |owner| {
        let builder = AstBuilder::new(&owner.allocator);
        let parser = Parser::new(&owner.allocator, &owner.source, ty);
        let ret = parser.parse_expression();
        match ret {
          Ok(expr) => {
            let program = builder.program(
              SPAN,
              SourceType::default().with_module(true),
              owner.source.as_str(),
              builder.vec(),
              None,
              builder.vec(),
              builder.vec1(builder.statement_expression(SPAN, expr)),
            );
            Ok(ProgramCellDependent { program })
          }
          Err(errors) => Err(BuildDiagnostic::from_oxc_diagnostics(
            errors,
            &source.clone(),
            filename,
            &Severity::Error,
          )),
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

  pub fn print_with(ast: &EcmaAst, options: PrintOptions) -> CodegenReturn {
    let (is_print_full_comments, legal_comments) = match options.comments {
      Either::Left(value) => (value, LegalComment::None),
      Either::Right(value) => (false, value),
    };
    CodeGenerator::new()
      .with_options(CodegenOptions {
        comments: is_print_full_comments,
        source_map_path: options.sourcemap.then(|| PathBuf::from(options.filename)),
        legal_comments,
        ..CodegenOptions::default()
      })
      .build(ast.program())
  }

  pub fn minify(
    source_text: &str,
    enable_sourcemap: bool,
    filename: &str,
  ) -> (String, Option<SourceMap>) {
    let allocator = Allocator::default();
    let program = Parser::new(&allocator, source_text, SourceType::default()).parse().program;
    let program = allocator.alloc(program);
    let options = MinifierOptions::default();
    let ret = Minifier::new(options).build(&allocator, program);
    let ret = Codegen::new()
      .with_options(CodegenOptions {
        source_map_path: enable_sourcemap.then(|| PathBuf::from(filename)),
        minify: true,
        ..CodegenOptions::default()
      })
      .with_mangler(ret.mangler)
      .build(program);
    (ret.code, ret.map)
  }
}

#[test]
fn basic_test() {
  let ast = EcmaCompiler::parse("", "const a = 1;".to_string(), SourceType::default()).unwrap();
  let code = EcmaCompiler::print(&ast, "", false).code;
  assert_eq!(code, "const a = 1;\n");
}

pub struct PrintOptions {
  pub comments: Either</* is print full comments */ bool, LegalComment>,
  pub filename: String,
  pub sourcemap: bool,
}
