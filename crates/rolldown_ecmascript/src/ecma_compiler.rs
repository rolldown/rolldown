use std::path::PathBuf;

use arcstr::ArcStr;
use oxc::{
  allocator::Allocator,
  ast::AstBuilder,
  codegen::{Codegen, CodegenOptions, CodegenReturn, CommentOptions, LegalComment},
  minifier::{Minifier, MinifierOptions},
  parser::{ParseOptions, Parser},
  span::{SPAN, SourceType},
};
use oxc_sourcemap::SourceMap;
use rolldown_error::{BuildDiagnostic, BuildResult, Severity};

use crate::ecma_ast::{
  EcmaAst,
  program_cell::{ProgramCell, ProgramCellDependent, ProgramCellOwner},
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
    Ok(EcmaAst { program: inner, source_type: ty })
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
    Ok(EcmaAst { program: inner, source_type: ty })
  }

  pub fn print_with(ast: &EcmaAst, options: PrintOptions) -> CodegenReturn {
    let legal =
      if options.print_legal_comments { LegalComment::Inline } else { LegalComment::None };
    Codegen::new()
      .with_options(CodegenOptions {
        comments: CommentOptions {
          normal: false,
          legal,
          // These option will be configurable when we begin to support `ignore-annotations`
          // https://esbuild.github.io/api/#ignore-annotations
          jsdoc: true,
          annotation: true,
        },
        initial_indent: options.initial_indent,
        source_map_path: options.sourcemap.then(|| PathBuf::from(options.filename)),
        ..CodegenOptions::default()
      })
      .build(ast.program())
  }

  pub fn minify(
    source_text: &str,
    source_type: SourceType,
    enable_sourcemap: bool,
    filename: &str,
    minifier_options: MinifierOptions,
    run_compress: bool,
    codegen_options: CodegenOptions,
  ) -> (String, Option<SourceMap>) {
    let allocator = Allocator::default();
    let mut program = Parser::new(&allocator, source_text, source_type).parse().program;
    let minifier = Minifier::new(minifier_options);
    let ret = if run_compress {
      minifier.minify(&allocator, &mut program)
    } else {
      minifier.dce(&allocator, &mut program)
    };
    let ret = Codegen::new()
      .with_options(CodegenOptions {
        source_map_path: enable_sourcemap.then(|| PathBuf::from(filename)),
        ..codegen_options
      })
      .with_scoping(ret.scoping)
      .build(&program);
    (ret.code, ret.map)
  }
}

#[test]
fn basic_test() {
  let ast = EcmaCompiler::parse("", "const a = 1;".to_string(), SourceType::default()).unwrap();
  let code = EcmaCompiler::print_with(&ast, PrintOptions::default()).code;
  assert_eq!(code, "const a = 1;\n");
}
#[derive(Debug, Default)]

pub struct PrintOptions {
  pub print_legal_comments: bool,
  pub filename: String,
  pub sourcemap: bool,
  pub initial_indent: u32,
}
