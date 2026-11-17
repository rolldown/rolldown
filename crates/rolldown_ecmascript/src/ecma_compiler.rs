use std::path::PathBuf;

use arcstr::ArcStr;
use oxc::{
  allocator::Allocator,
  ast::{
    ast::{Program, Statement},
    builder::AstBuilder,
  },
  codegen::{Codegen, CodegenOptions, CodegenReturn, CommentOptions, LegalComment},
  minifier::{CompressOptions, Compressor, ManglePropertyCache, MinifierOptions, PropertyMangler},
  parser::{ParseOptions, Parser},
  semantic::{SemanticBuilder, Stats},
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

  /// The returned map borrows `source_text` (its `sourcesContent` and token names are slices
  /// of it) — callers that keep it beyond the borrow must `into_owned` it; callers that only
  /// feed it to `collapse_sourcemaps` can use it as-is and skip that copy.
  /// The property-mangle cache is present when property mangling ran.
  #[expect(clippy::too_many_arguments)]
  pub fn dce_or_minify<'a>(
    allocator: &'a Allocator,
    source_text: &'a str,
    source_type: SourceType,
    enable_sourcemap: bool,
    filename: &str,
    compress: bool,
    minify_options: MinifierOptions,
    codegen_options: CodegenOptions,
  ) -> BuildResult<(String, Option<SourceMap<'a>>, Option<ManglePropertyCache>)> {
    let ret = Parser::new(allocator, source_text, source_type)
      .with_options(ParseOptions { preserve_parens: false, ..ParseOptions::default() })
      .parse();
    if ret.fatal_error || !ret.diagnostics.is_empty() {
      return Err(
        BuildDiagnostic::from_oxc_diagnostics(
          ret.diagnostics,
          &ArcStr::from(source_text),
          filename,
          Severity::Error,
          EventKind::ParseError,
        )
        .into(),
      );
    }
    let mut program = ret.program;
    // Count once so both the compress/DCE semantic and the later mangle
    // semantic reserve their tables. `Minifier::minify` / `Minifier::dce`
    // build semantic with no stats, so large chunks grow those tables
    // (oxc measures that growth at up to 30% of semantic time).
    // Mangle runs on the post-compress AST; reusing the pre-compress
    // counts only over-reserves, which does not reallocate.
    let stats = Stats::count(&program);
    let minified = minify_program(allocator, &mut program, compress, minify_options, stats);
    let ret = Codegen::new()
      .with_options(CodegenOptions {
        source_map_path: enable_sourcemap.then(|| PathBuf::from(filename)),
        ..codegen_options
      })
      .with_scoping(minified.scoping)
      .with_private_member_mappings(minified.class_private_mappings)
      .build(&program);
    Ok((ret.code, ret.map, minified.property_mangle_cache))
  }
}

/// Same pipeline as `oxc_minifier::Minifier::build`, but the compress/DCE
/// and mangle `SemanticBuilder`s are sized with `stats`.
fn minify_program<'a>(
  allocator: &'a Allocator,
  program: &mut Program<'a>,
  compress: bool,
  options: MinifierOptions,
  stats: Stats,
) -> oxc::minifier::MinifierReturn {
  let MinifierOptions { mangle, mangle_properties, compress: compress_options } = options;

  let property_mangle_cache = mangle_properties.map(|options| {
    let mut mangler = PropertyMangler::new_in(options, allocator);
    mangler.collect(program);
    mangler.assign();
    mangler.rewrite(program);
    mangler.into_cache()
  });

  let mut iterations = 0u8;
  if let Some(compress_options) = compress_options {
    let semantic = SemanticBuilder::new().with_stats(stats).build(program).semantic;
    let scoping = semantic.into_scoping();
    let compressor = Compressor::new(allocator);
    iterations = if compress {
      compressor.build_with_scoping(program, scoping, compress_options)
    } else {
      // Match `Minifier::dce`: keep the caller's target and treeshake, but
      // otherwise use the DCE-only compress set.
      let options = CompressOptions {
        target: compress_options.target,
        treeshake: compress_options.treeshake,
        ..CompressOptions::dce()
      };
      compressor.dead_code_elimination_with_scoping(program, scoping, options)
    };
  }

  let (scoping, class_private_mappings) = match mangle {
    Some(options) => {
      let mut semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .with_class_table(true)
        .with_stats(stats)
        .build(program)
        .semantic;
      let class_private_mappings = oxc::mangler::Mangler::default()
        .with_options(options)
        .build_with_semantic(&mut semantic, program);
      (Some(semantic.into_scoping()), Some(class_private_mappings))
    }
    None => (None, None),
  };

  oxc::minifier::MinifierReturn {
    scoping,
    class_private_mappings,
    property_mangle_cache,
    iterations,
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
