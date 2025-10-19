use std::path::Path;

use itertools::Itertools;
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
use rolldown_error::{BuildDiagnostic, BuildResult, Severity};

use crate::types::oxc_parse_type::OxcParseType;

use super::parse_to_ecma_ast::ParseToEcmaAstResult;
use super::tweak_ast_for_scanning::PreProcessor;

#[derive(Default)]
pub struct PreProcessEcmaAst {
  /// Semantic statistics.
  stats: Stats,
}

impl PreProcessEcmaAst {
  pub fn build(
    &mut self,
    mut ast: EcmaAst,
    path: &str,
    parsed_type: &OxcParseType,
    replace_global_define_config: Option<&ReplaceGlobalDefinesConfig>,
    bundle_options: &NormalizedBundlerOptions,
    has_lazy_export: bool,
  ) -> BuildResult<ParseToEcmaAstResult> {
    let source = ast.source().clone();
    // Step 1: Build initial semantic data and check for semantic errors.
    let semantic_ret = ast.program.with_dependent(|_owner, dep| {
      SemanticBuilder::new().with_check_syntax_error(true).build(&dep.program)
    });

    let (errors, warnings): (Vec<_>, Vec<_>) =
      semantic_ret.errors.into_iter().partition(|w| w.severity == OxcSeverity::Error);

    let warnings = if errors.is_empty() {
      BuildDiagnostic::from_oxc_diagnostics(warnings, &source, path, &Severity::Warning)
    } else {
      return Err(BuildDiagnostic::from_oxc_diagnostics(errors, &source, path, &Severity::Error))?;
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
    if !matches!(parsed_type, OxcParseType::Js)
      || bundle_options.transform_options.env.regexp.set_notation
    {
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        let transform_options = &bundle_options.transform_options;
        let scoping = self.recreate_scoping(&mut scoping, program, false);
        let ret = Transformer::new(allocator, Path::new(path), transform_options)
          .build_with_scoping(scoping, program);
        // TODO: emit diagnostic, aiming to pass more tests,
        // we ignore warning for now
        if ret.errors.iter().any(|error| error.severity == OxcSeverity::Error) {
          let errors = ret
            .errors
            .into_iter()
            .filter(|item| matches!(item.severity, OxcSeverity::Error))
            .collect_vec();
          return Err(BuildDiagnostic::from_oxc_diagnostics(
            errors,
            &source,
            path,
            &Severity::Error,
          ));
        }
        Ok(())
      })?;
    }

    // Step 4: Run inject plugin.
    if !bundle_options.inject.is_empty() {
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        let new_scoping = self.recreate_scoping(&mut scoping, program, false);
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
        let scoping = self.recreate_scoping(&mut scoping, program, false);
        // NOTE: `CompressOptions::dead_code_elimination` will remove `ParenthesizedExpression`s from the AST.
        let options = CompressOptions {
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
      self.recreate_scoping(&mut None, program, true)
    });

    Ok(ParseToEcmaAstResult { ast, scoping, has_lazy_export, warnings })
  }

  fn recreate_scoping(
    &mut self,
    scoping: &mut Option<Scoping>,
    program: &Program<'_>,
    with_scope_tree_child_ids: bool,
  ) -> Scoping {
    if let Some(scoping) = scoping.take() {
      return scoping;
    }
    let ret = SemanticBuilder::new()
      // Required by `module.scope.get_child_ids` in `crates/rolldown/src/utils/renamer.rs`.
      .with_scope_tree_child_ids(with_scope_tree_child_ids)
      // Preallocate memory for the underlying data structures.
      .with_stats(self.stats)
      .build(program)
      .semantic;
    self.stats = ret.stats();
    ret.into_scoping()
  }
}
