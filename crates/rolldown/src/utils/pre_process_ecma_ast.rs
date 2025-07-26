use std::path::Path;

use itertools::Itertools;
use oxc::ast_visit::VisitMut;
use oxc::diagnostics::Severity as OxcSeverity;
use oxc::minifier::{CompressOptions, Compressor, TreeShakeOptions};
use oxc::semantic::{SemanticBuilder, Stats};
use oxc::transformer::Transformer;
use oxc::transformer_plugins::{
  InjectGlobalVariables, ReplaceGlobalDefines, ReplaceGlobalDefinesConfig,
};

use rolldown_common::NormalizedBundlerOptions;
use rolldown_ecmascript::{EcmaAst, WithMutFields};
use rolldown_error::{BuildDiagnostic, BuildResult, Severity};

use crate::types::oxc_parse_type::OxcParseType;

use super::ecma_visitors::EnsureSpanUniqueness;
use super::parse_to_ecma_ast::ParseToEcmaAstResult;
use super::tweak_ast_for_scanning::PreProcessor;

#[derive(Default)]
pub struct PreProcessEcmaAst {
  /// Only recreate semantic data if ast is changed.
  ast_changed: bool,

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
    let mut warning = vec![];
    let source = ast.source().clone();
    // Build initial semantic data and check for semantic errors.
    let semantic_ret =
      ast.program.with_mut(|WithMutFields { program, .. }| SemanticBuilder::new().build(program));
    if !semantic_ret.errors.is_empty() {
      warning.extend(BuildDiagnostic::from_oxc_diagnostics(
        semantic_ret.errors,
        &source,
        path,
        &Severity::Warning,
      ));
    }

    self.stats = semantic_ret.semantic.stats();
    let scoping = semantic_ret.semantic.into_scoping();

    let mut scoping = ast.program.with_mut(|fields| {
      let WithMutFields { allocator, program, .. } = fields;
      // Use built-in define plugin.
      if let Some(replace_global_define_config) = replace_global_define_config {
        let ret = ReplaceGlobalDefines::new(allocator, replace_global_define_config.clone())
          .build(scoping, program);
        self.ast_changed = true;
        ret.scoping
      } else {
        scoping
      }
    });
    // Transform TypeScript and jsx.
    // Note: Currently, oxc_transform supports es syntax up to ES2024 (unicode-sets-regex).
    if !matches!(parsed_type, OxcParseType::Js)
      || bundle_options.transform_options.env.regexp.set_notation
    {
      let ret = ast.program.with_mut(|fields| {
        let transform_options = &bundle_options.transform_options;

        Transformer::new(fields.allocator, Path::new(path), transform_options)
          .build_with_scoping(scoping, fields.program)
      });

      // TODO: emit diagnostic, aiming to pass more tests,
      // we ignore warning for now
      let errors = ret
        .errors
        .into_iter()
        .filter(|item| matches!(item.severity, OxcSeverity::Error))
        .collect_vec();
      if !errors.is_empty() {
        Err(BuildDiagnostic::from_oxc_diagnostics(errors, &source, path, &Severity::Error))?;
      }

      scoping = ret.scoping;
      self.ast_changed = true;
    }

    ast.program.with_mut(|fields| {
      let WithMutFields { allocator, program, .. } = fields;

      if !bundle_options.inject.is_empty() {
        // if the define replace something, we need to recreate the semantic data.
        // to correct the `root_unresolved_references`
        // https://github.com/oxc-project/oxc/blob/0136431b31a1d4cc20147eb085d9314b224cc092/crates/oxc_transformer/src/plugins/inject_global_variables.rs#L184-L184
        // TODO: real ast_changed hint
        let semantic_ret = SemanticBuilder::new().with_stats(self.stats).build(program);
        scoping = semantic_ret.semantic.into_scoping();
        let ret = InjectGlobalVariables::new(
          allocator,
          bundle_options.oxc_inject_global_variables_config.clone(),
        )
        .build(scoping, program);
        scoping = ret.scoping;
        self.ast_changed = true;
      }

      // avoid DCE for lazy export
      if bundle_options.treeshake.is_some() && !has_lazy_export {
        // Perform dead code elimination.
        // NOTE: `CompressOptions::dead_code_elimination` will remove `ParenthesizedExpression`s from the AST.
        let options = CompressOptions {
          treeshake: TreeShakeOptions::from(&bundle_options.treeshake),
          join_vars: false,
          sequences: false,
          ..CompressOptions::safest()
        };
        let compressor = Compressor::new(allocator);
        if self.ast_changed {
          let semantic_ret = SemanticBuilder::new().with_stats(self.stats).build(program);
          scoping = semantic_ret.semantic.into_scoping();
        }
        compressor.dead_code_elimination_with_scoping(program, scoping, options);
      }
    });

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator, bundle_options.keep_names);
      pre_processor.visit_program(fields.program);
    });

    ast.program.with_mut(|fields| {
      EnsureSpanUniqueness::new().visit_program(fields.program);
    });

    // NOTE: Recreate semantic data because AST is changed in the transformations above.
    let scoping = ast.program.with_dependent(|_owner, dep| {
      SemanticBuilder::new()
        // Required by `module.scope.get_child_ids` in `crates/rolldown/src/utils/renamer.rs`.
        .with_scope_tree_child_ids(true)
        // Preallocate memory for the underlying data structures.
        .with_stats(self.stats)
        .build(&dep.program)
        .semantic
        .into_scoping()
    });

    Ok(ParseToEcmaAstResult { ast, scoping, has_lazy_export, warning })
  }
}
