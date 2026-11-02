use std::path::PathBuf;

use arcstr::ArcStr;
use oxc::{
  allocator::Allocator,
  ast::{
    ast::{Program, Statement},
    builder::AstBuilder,
  },
  codegen::{Codegen, CodegenOptions, CodegenReturn, CommentOptions, LegalComment},
  minifier::{Minifier, MinifierOptions},
  parser::{ParseOptions, Parser},
  span::{SPAN, SourceType},
};
use rolldown_error::{BuildDiagnostic, BuildResult, EventKind, Severity};

use crate::ecma_ast::{
  EcmaAst,
  program_cell::{ProgramCell, ProgramCellDependent, ProgramCellOwner},
};
pub struct EcmaCompiler;

impl EcmaCompiler {
  pub fn parse(id: &str, source: impl Into<ArcStr>, ty: SourceType) -> BuildResult<EcmaAst> {
    let source: ArcStr = source.into();
    let allocator = oxc::allocator::Allocator::default();
    let inner =
      ProgramCell::try_new(ProgramCellOwner { source: source.clone(), allocator }, |owner| {
        let parser = Parser::new(&owner.allocator, &owner.source, ty).with_options(ParseOptions {
          allow_return_outside_function: true,
          preserve_parens: false,
          ..ParseOptions::default()
        });
        let ret = parser.parse();
        if ret.fatal_error || !ret.diagnostics.is_empty() {
          Err(BuildDiagnostic::from_oxc_diagnostics(
            ret.diagnostics,
            &source.clone(),
            id,
            Severity::Error,
            EventKind::ParseError,
          ))
        } else {
          Ok(ProgramCellDependent { program: ret.program })
        }
      })?;
    Ok(EcmaAst { program: inner, source_type: ty })
  }

  pub fn parse_expr_as_program(
    id: &str,
    source: impl Into<ArcStr>,
    ty: SourceType,
  ) -> BuildResult<EcmaAst> {
    let source: ArcStr = source.into();
    let allocator = oxc::allocator::Allocator::default();
    let inner =
      ProgramCell::try_new(ProgramCellOwner { source: source.clone(), allocator }, |owner| {
        let builder = AstBuilder::new(&owner.allocator);
        let parser = Parser::new(&owner.allocator, &owner.source, ty)
          .with_options(ParseOptions { preserve_parens: false, ..ParseOptions::default() });
        let ret = parser.parse_expression();
        match ret {
          Ok(expr) => {
            let program = Program::new(
              SPAN,
              SourceType::default().with_module(true),
              owner.source.as_str(),
              [],
              None,
              [],
              [Statement::new_expression_statement(SPAN, expr, &builder)],
              &builder,
            );
            Ok(ProgramCellDependent { program })
          }
          Err(errors) => Err(BuildDiagnostic::from_oxc_diagnostics(
            errors,
            &source.clone(),
            id,
            Severity::Error,
            EventKind::ParseError,
          )),
        }
      })?;
    Ok(EcmaAst { program: inner, source_type: ty })
  }

  pub fn print_with(ast: &EcmaAst, options: PrintOptions) -> CodegenReturn<'_> {
    let legal = if options.comments.legal { LegalComment::Inline } else { LegalComment::None };
    Codegen::new()
      .with_options(CodegenOptions {
        comments: CommentOptions {
          normal: false,
          legal,
          jsdoc: options.comments.jsdoc,
          annotation: options.comments.annotation,
        },
        initial_indent: options.initial_indent,
        ascii_only: options.ascii_only,
        source_map_path: options.sourcemap.then(|| PathBuf::from(options.filename)),
        ..CodegenOptions::default()
      })
      .build(ast.program())
  }

  /// Shared optimization boundary for assembled chunks and parser-produced final hook text.
  /// The caller retains ownership and handles printing/provenance after this returns.
  pub fn minify_program<'a>(
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    options: MinifierOptions,
    compress: bool,
    preserve_literal_spans: bool,
  ) -> oxc::minifier::MinifierReturn {
    let minifier = Minifier::new(options).with_literal_spans(preserve_literal_spans);
    if compress { minifier.minify(allocator, program) } else { minifier.dce(allocator, program) }
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
  pub comments: PrintCommentsOptions,
  pub filename: String,
  pub sourcemap: bool,
  pub initial_indent: u32,
  pub ascii_only: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PrintCommentsOptions {
  pub legal: bool,
  pub annotation: bool,
  pub jsdoc: bool,
}
