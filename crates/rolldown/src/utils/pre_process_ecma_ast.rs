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
use rolldown_ecmascript_utils::contains_script_closing_tag;
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

    self.stats = semantic_ret.semantic.stats();
    let scoping = semantic_ret.semantic.into_scoping();
    
    let mut warnings = if errors.is_empty() {
      BuildDiagnostic::from_oxc_diagnostics(
        warnings,
        &source,
        resolved_id,
        Severity::Warning,
        EventKind::ParseError,
      )
    } else {
      // Process export undefined errors to add similar name suggestions
      let augmented_errors = augment_export_undefined_errors(errors, &scoping, &source, resolved_id);
      return Err(augmented_errors)?;
    };

    let mut scoping = Some(scoping);

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

    if is_not_js
      || bundle_options.transform_options.should_transform_js()
      // Run transformer on JS files containing `</script` to handle tagged template literals.
      || contains_script_closing_tag(ast.source().as_bytes())
    {
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
            Severity::Error,
            EventKind::TransformError,
          )));
        }
        warnings.extend(BuildDiagnostic::from_oxc_diagnostics(
          transformer_warnings,
          &source,
          resolved_id,
          Severity::Warning,
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

/// Augment export undefined errors from oxc with similar name suggestions
/// Augment export undefined errors from oxc with similar name suggestions
fn augment_export_undefined_errors(
  errors: Vec<oxc::diagnostics::OxcDiagnostic>,
  scoping: &Scoping,
  source: &arcstr::ArcStr,
  resolved_id: &str,
) -> BatchedBuildDiagnostic {
  use arcstr::ArcStr;
  use oxc::span::Span;
  
  let root_scope_id = scoping.root_scope_id();
  let bindings = scoping.get_bindings(root_scope_id);
  let binding_names: Vec<&str> = bindings.keys().map(|s| &**s).collect();
  
  let mut diagnostics = Vec::new();
  
  for mut error in errors {
    // Check if this is an "Export is not defined" error
    let error_message = error.message.to_string();
    if error_message.contains("is not defined") && error_message.starts_with("Export '") {
      // Extract the undefined export name from the message
      // Message format: "Export 'name' is not defined"
      if let Some(start) = error_message.find('\'') {
        if let Some(end) = error_message[start + 1..].find('\'') {
          let name = &error_message[start + 1..start + 1 + end];
          let similar_names = rolldown_utils::string_similarity::find_similar_str(
            name,
            binding_names.clone(),
            3,
          )
          .into_iter()
          .map(|s| s.to_string())
          .collect::<Vec<_>>();
          
          // Get the span from the error labels
          if let Some(labels) = &error.labels {
            if let Some(label) = labels.first() {
              let offset = label.offset();
              let len = label.len();
              let span = Span::new(offset as u32, (offset + len) as u32);
              
              diagnostics.push(BuildDiagnostic::export_undefined_variable(
                resolved_id.to_string(),
                source.clone(),
                span,
                ArcStr::from(name),
                similar_names,
              ));
              continue;
            }
          }
        }
      }
    }
    
    // For non-export-undefined errors, convert normally
    diagnostics.push(BuildDiagnostic::oxc_error(
      source.clone(),
      resolved_id.to_string(),
      error.help.take().unwrap_or_default().into(),
      error.message.to_string(),
      error.labels.take().unwrap_or_default(),
      EventKind::ParseError,
    ));
  }
  
  BatchedBuildDiagnostic::from(diagnostics)
}
