use std::path::Path;

use oxc::ast::ast::Program;
use oxc::ast_visit::VisitMut;
use oxc::diagnostics::Severity as OxcSeverity;
use oxc::minifier::{CompressOptions, Compressor, TreeShakeOptions};
use oxc::semantic::{Scoping, SemanticBuilder, Stats};
use oxc::transformer::Transformer;
use oxc::transformer_plugins::{
  InjectGlobalVariables, ReplaceGlobalDefines, ReplaceGlobalDefinesConfig,
};

use rolldown_common::NormalizedBundlerOptions;
use rolldown_ecmascript::{EcmaAst, WithMutFields};
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic, BuildResult, EventKind, Severity};

use crate::types::oxc_parse_type::OxcParseType;

use super::parse_to_ecma_ast::ParseToEcmaAstResult;
use super::tweak_ast_for_scanning::PreProcessor;

#[derive(Default)]
pub struct PreProcessEcmaAst {
  /// Semantic statistics.
  stats: Stats,
}

impl PreProcessEcmaAst {
  #[expect(clippy::too_many_arguments)]
  pub fn build(
    &mut self,
    mut ast: EcmaAst,
    stable_id: &str,
    resolved_id: &str,
    parsed_type: &OxcParseType,
    replace_global_define_config: Option<&ReplaceGlobalDefinesConfig>,
    bundle_options: &NormalizedBundlerOptions,
    has_lazy_export: bool,
  ) -> BuildResult<ParseToEcmaAstResult> {
    let source = ast.source().clone();

    // Step 0: Move directive comments attached to 0 so that it's not removed when the directives are removed
    if !ast.program().directives.is_empty() && !ast.program().comments.is_empty() {
      ast.program.with_mut(|WithMutFields { program, .. }| {
        let mut i = 0;
        for directive in &program.directives {
          while i < program.comments.len() {
            let comment = &mut program.comments[i];
            if comment.attached_to == directive.span.start {
              comment.attached_to = 0;
            } else if comment.attached_to > directive.span.start {
              break;
            }
            i += 1;
          }
        }
      });
    }

    // Step 1: Build initial semantic data and check for semantic errors.
    let semantic_ret = ast.program.with_dependent(|_owner, dep| {
      SemanticBuilder::new().with_check_syntax_error(true).build(&dep.program)
    });

    let (errors, warnings): (Vec<_>, Vec<_>) =
      semantic_ret.errors.into_iter().partition(|w| w.severity == OxcSeverity::Error);

    let mut warnings = if errors.is_empty() {
      BuildDiagnostic::from_oxc_diagnostics(
        warnings,
        &source,
        resolved_id,
        &Severity::Warning,
        EventKind::ParseError,
      )
    } else {
      return Err(BuildDiagnostic::from_oxc_diagnostics(
        errors,
        &source,
        resolved_id,
        &Severity::Error,
        EventKind::ParseError,
      ))?;
    };

    self.stats = semantic_ret.semantic.stats();
    let mut scoping = Some(semantic_ret.semantic.into_scoping());

    // Step 2: Run define plugin.
    if let Some(replace_global_define_config) = replace_global_define_config {
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        let ret = ReplaceGlobalDefines::new(allocator, replace_global_define_config.clone())
          .build(scoping.take().unwrap(), program);
        if !ret.changed {
          scoping = Some(ret.scoping);
        }
      });
    }

    // Step 3: Transform TypeScript and jsx.
    // Note: Currently, oxc_transform supports es syntax up to ES2024 (unicode-sets-regex).
    let is_not_js = !matches!(parsed_type, OxcParseType::Js);
    if is_not_js || bundle_options.transform_options.should_transform_js() {
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        // Pass file path only for non-JS modules (TS/TSX/JSX) to enable tsconfig discovery.
        // For plain JS files, we skip tsconfig lookup since they don't need TS-specific transformations.
        let transform_options = bundle_options
          .transform_options
          .options_for_file(is_not_js.then_some(Path::new(resolved_id)), &mut warnings)?;

        let scoping = self.recreate_scoping(&mut scoping, program);
        let ret = Transformer::new(allocator, Path::new(stable_id), &transform_options)
          .build_with_scoping(scoping, program);

        let (errors, transformer_warnings): (Vec<_>, Vec<_>) =
          ret.errors.into_iter().partition(|error| error.severity == OxcSeverity::Error);
        if !errors.is_empty() {
          return Err(BatchedBuildDiagnostic::from(BuildDiagnostic::from_oxc_diagnostics(
            errors,
            &source,
            resolved_id,
            &Severity::Error,
            EventKind::TransformError,
          )));
        }
        warnings.extend(BuildDiagnostic::from_oxc_diagnostics(
          transformer_warnings,
          &source,
          resolved_id,
          &Severity::Warning,
          EventKind::ToleratedTransform,
        ));
        Ok(())
      })?;
    }

    // Step 4: Run inject plugin.
    if !bundle_options.inject.is_empty() {
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        let new_scoping = self.recreate_scoping(&mut scoping, program);
        let inject_config = bundle_options.oxc_inject_global_variables_config.clone();
        let ret = InjectGlobalVariables::new(allocator, inject_config).build(new_scoping, program);
        if !ret.changed {
          scoping = Some(ret.scoping);
        }
      });
    }

    // Step 5: Run DCE.
    // Avoid DCE for lazy export.
    if bundle_options.treeshake.is_some() && !has_lazy_export {
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        let scoping = self.recreate_scoping(&mut scoping, program);
        // NOTE: `CompressOptions::dead_code_elimination` will remove `ParenthesizedExpression`s from the AST.
        let options = CompressOptions {
          target: bundle_options.transform_options.target.clone(),
          treeshake: TreeShakeOptions::from(&bundle_options.treeshake),
          ..CompressOptions::dce()
        };
        Compressor::new(allocator).dead_code_elimination_with_scoping(program, scoping, options);
      });
    }

    // Step 6: Modify AST for Rolldown.
    let scoping = ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
      let mut pre_processor = PreProcessor::new(allocator, bundle_options.keep_names);
      pre_processor.visit_program(program);
      self.recreate_scoping(&mut None, program)
    });

    Ok(ParseToEcmaAstResult { ast, scoping, has_lazy_export, warnings })
  }

  fn recreate_scoping(&mut self, scoping: &mut Option<Scoping>, program: &Program<'_>) -> Scoping {
    if let Some(scoping) = scoping.take() {
      return scoping;
    }
    let ret = SemanticBuilder::new()
      // Preallocate memory for the underlying data structures.
      .with_stats(self.stats)
      .build(program)
      .semantic;
    self.stats = ret.stats();
    ret.into_scoping()
  }
}
