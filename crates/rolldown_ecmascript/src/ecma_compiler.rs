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
use rolldown_error::{BuildDiagnostic, BuildResult, EventKind, Severity};

use crate::ecma_ast::{
  EcmaAst,
  program_cell::{ProgramCell, ProgramCellDependent, ProgramCellOwner},
};
pub struct EcmaCompiler;

impl EcmaCompiler {
  pub fn parse(id: &str, source: impl Into<ArcStr>, ty: SourceType) -> BuildResult<EcmaAst> {
    let source: ArcStr = source.into();
    let allocator = allocator_for_source(&source);
    let inner =
      ProgramCell::try_new(ProgramCellOwner { source: source.clone(), allocator }, |owner| {
        let parser = Parser::new(&owner.allocator, &owner.source, ty).with_options(ParseOptions {
          allow_return_outside_function: true,
          ..ParseOptions::default()
        });
        let ret = parser.parse();
        if ret.panicked || !ret.diagnostics.is_empty() {
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
    let allocator = allocator_for_source(&source);
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
        source_map_path: options.sourcemap.then(|| PathBuf::from(options.filename)),
        ..CodegenOptions::default()
      })
      .build(ast.program())
  }

  #[expect(clippy::too_many_arguments)]
  pub fn dce_or_minify(
    allocator: &Allocator,
    source_text: &str,
    source_type: SourceType,
    enable_sourcemap: bool,
    filename: &str,
    compress: bool,
    minify_options: MinifierOptions,
    codegen_options: CodegenOptions,
  ) -> (String, Option<SourceMap<'static>>) {
    let mut program = Parser::new(allocator, source_text, source_type).parse().program;
    let minifier = Minifier::new(minify_options);
    let ret = if compress {
      minifier.minify(allocator, &mut program)
    } else {
      minifier.dce(allocator, &mut program)
    };
    let ret = Codegen::new()
      .with_options(CodegenOptions {
        source_map_path: enable_sourcemap.then(|| PathBuf::from(filename)),
        ..codegen_options
      })
      .with_scoping(ret.scoping)
      .with_private_member_mappings(ret.class_private_mappings)
      .build(&program);
    (ret.code, ret.map.map(SourceMap::into_owned))
  }
}

/// Create an Oxc AST allocator with initial capacity derived from the source shape.
pub fn allocator_for_source(source: &str) -> Allocator {
  initial_ast_capacity(source).map_or_else(Allocator::default, Allocator::with_capacity)
}

fn initial_ast_capacity(source: &str) -> Option<usize> {
  // Oxc's default first chunk is 16 KiB, which over-allocates heavily for
  // projects containing thousands of small modules. Source size is a useful
  // baseline, while import-dense modules need extra room for their many short
  // AST nodes. Larger estimates use Oxc's default growth policy to avoid a
  // large upfront allocation for sources containing mostly comments or data.
  const BYTES_PER_SOURCE_BYTE: usize = 8;
  const BYTES_PER_IMPORT: usize = 1024;
  const FIXED_HEADROOM: usize = 2048;
  const MAX_INITIAL_CAPACITY: usize = 64 * 1024;

  if source.is_empty() {
    return Some(0);
  }

  let base_capacity =
    source.len().saturating_mul(BYTES_PER_SOURCE_BYTE).saturating_add(FIXED_HEADROOM);
  if base_capacity > MAX_INITIAL_CAPACITY {
    return None;
  }

  let import_count = memchr::memmem::find_iter(source.as_bytes(), b"import").count();
  let capacity = base_capacity.saturating_add(import_count.saturating_mul(BYTES_PER_IMPORT));
  (capacity <= MAX_INITIAL_CAPACITY).then_some(capacity)
}

#[test]
fn basic_test() {
  let ast = EcmaCompiler::parse("", "const a = 1;".to_string(), SourceType::default()).unwrap();
  let code = EcmaCompiler::print_with(&ast, PrintOptions::default()).code;
  assert_eq!(code, "const a = 1;\n");
}

#[test]
fn initial_ast_capacity_accounts_for_source_shape_and_limit() {
  let source = "import a from 'a';\nimport b from 'b';\nexport { a, b };";
  assert_eq!(initial_ast_capacity(source), Some(source.len() * 8 + 2 * 1024 + 2048));
  assert_eq!(initial_ast_capacity(""), Some(0));
  assert_eq!(initial_ast_capacity(&"x".repeat(8 * 1024)), None);
}
#[derive(Debug, Default)]

pub struct PrintOptions {
  pub comments: PrintCommentsOptions,
  pub filename: String,
  pub sourcemap: bool,
  pub initial_indent: u32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PrintCommentsOptions {
  pub legal: bool,
  pub annotation: bool,
  pub jsdoc: bool,
}
