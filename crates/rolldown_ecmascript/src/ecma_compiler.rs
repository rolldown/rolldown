use std::path::PathBuf;

use arcstr::ArcStr;
use oxc::{
  allocator::Allocator,
  ast::{
    ast::{Program, Statement},
    builder::AstBuilder,
  },
  codegen::{Codegen, CodegenOptions, CodegenReturn, CommentOptions, LegalComment},
  minifier::{ManglePropertyCache, Minifier, MinifierOptions},
  parser::{ParseOptions, Parser},
  span::{SPAN, SourceType},
};
use oxc_sourcemap::SourceMap;
use rolldown_error::{BuildDiagnostic, BuildResult, EventKind, Severity};
use rolldown_utils::poc_prof::{Phase, Timer};

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
    let minifier = Minifier::new(minify_options);
    let ret = if compress {
      minifier.minify(allocator, &mut program)
    } else {
      minifier.dce(allocator, &mut program)
    };
    let property_mangle_cache = ret.property_mangle_cache;
    let ret = Codegen::new()
      .with_options(CodegenOptions {
        source_map_path: enable_sourcemap.then(|| PathBuf::from(filename)),
        ..codegen_options
      })
      .with_scoping(ret.scoping)
      .with_private_member_mappings(ret.class_private_mappings)
      .build(&program);
    Ok((ret.code, ret.map, property_mangle_cache))
  }

  /// `Minifier::minify`/`dce` with OXC's public stages separately timed for the PoC profiler.
  pub fn minify_profiled<'a>(
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    minify_options: MinifierOptions,
    full: bool,
    preserve_literal_spans: bool,
  ) -> oxc::minifier::MinifierReturn {
    use oxc::{
      mangler::Mangler,
      minifier::{CompressOptions, Compressor},
      semantic::SemanticBuilder,
    };

    if minify_options.mangle_properties.is_some() {
      let minifier = Minifier::new(minify_options).with_literal_spans(preserve_literal_spans);
      return if full {
        minifier.minify(allocator, program)
      } else {
        minifier.dce(allocator, program)
      };
    }
    let MinifierOptions { mangle, compress, .. } = minify_options;
    let (stats, iterations) = compress
      .map(|options| {
        let t = Timer::start();
        let semantic = SemanticBuilder::new().build(program).semantic;
        t.stop(Phase::MinifySemantic);
        let stats = semantic.stats();
        let scoping = semantic.into_scoping();
        let compressor = Compressor::new(allocator).with_literal_spans(preserve_literal_spans);
        let t = Timer::start();
        let iterations = if full {
          compressor.build_with_scoping(program, scoping, options)
        } else {
          let options = CompressOptions {
            target: options.target,
            treeshake: options.treeshake,
            ..CompressOptions::dce()
          };
          compressor.dead_code_elimination_with_scoping(program, scoping, options)
        };
        t.stop(Phase::MinifyCompress);
        (Some(stats), iterations)
      })
      .unwrap_or_default();
    let (scoping, class_private_mappings) = mangle
      .map(|options| {
        let mut builder = SemanticBuilder::new().with_build_nodes(true).with_class_table(true);
        if let Some(stats) = stats {
          builder = builder.with_stats(stats);
        }
        let t = Timer::start();
        let mut semantic = builder.build(program).semantic;
        t.stop(Phase::MinifyMangleSemantic);
        let t = Timer::start();
        let class_private_mappings =
          Mangler::default().with_options(options).build_with_semantic(&mut semantic, program);
        t.stop(Phase::MinifyMangle);
        (semantic.into_scoping(), class_private_mappings)
      })
      .map_or((None, None), |(scoping, mappings)| (Some(scoping), Some(mappings)));
    oxc::minifier::MinifierReturn {
      scoping,
      class_private_mappings,
      property_mangle_cache: None,
      iterations,
    }
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
