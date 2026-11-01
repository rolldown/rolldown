use std::path::Path;

use arcstr::ArcStr;
use oxc::ast::ast::CommentContent;
use oxc::ast::ast::Program;
use oxc::ast::ast::{Declaration, ExportDefaultDeclarationKind, Statement};
use oxc::ast_visit::{VisitJs, VisitJsMut, walk_js};
use oxc::diagnostics::Severity as OxcSeverity;
use oxc::minifier::{CompressOptions, Compressor, TreeShakeOptions};
use oxc::semantic::{Scoping, Stats};
use oxc::syntax::symbol::SymbolFlags;
use oxc::transformer::Transformer;
use oxc::transformer_plugins::{
  InjectGlobalVariables, ReplaceGlobalDefines, ReplaceGlobalDefinesConfig,
};
use oxc_str::CompactStr;

use rolldown_common::{ConstExportMeta, ConstantValue, NormalizedBundlerOptions};
use rolldown_ecmascript::{EcmaAst, WithMutFields, semantic_builder_for_transform};
use rolldown_ecmascript_utils::contains_script_closing_tag;
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic, BuildResult, EventKind, Severity};
use rustc_hash::{FxHashMap, FxHashSet};

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
    should_warn_on_invalid_annotation: bool,
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
      semantic_builder_for_transform()
        .with_check_syntax_error(true)
        .build(&dep.program)
    });

    let (errors, warnings): (Vec<_>, Vec<_>) = semantic_ret
      .diagnostics
      .into_iter()
      .partition(|w| w.severity == OxcSeverity::Error);

    let mut warnings = if errors.is_empty() {
      BuildDiagnostic::from_oxc_diagnostics(
        warnings,
        &source,
        resolved_id,
        Severity::Warning,
        EventKind::ParseError,
      )
    } else {
      return Err(BuildDiagnostic::from_oxc_diagnostics(
        errors,
        &source,
        resolved_id,
        Severity::Error,
        EventKind::ParseError,
      ))?;
    };
    // Surface invalid annotations flagged by oxc (issue #8898).
    // oxc marks misplaced `PURE` and `NO_SIDE_EFFECTS` comments with their
    // respective `NotApplied` variants when the parser cannot apply them.
    // Aligns with Rollup's `INVALID_ANNOTATION` log code.
    if should_warn_on_invalid_annotation {
      warnings.extend(ast.program.with_dependent(|_owner, dep| {
        invalid_annotation_warnings(&dep.program, &source, resolved_id)
      }));
    }

    self.stats = semantic_ret.semantic.stats();
    let mut scoping = Some(semantic_ret.semantic.into_scoping());
    let module_resources = ast.program().body.iter().any(|statement| {
      matches!(statement, Statement::VariableDeclaration(declaration)
        if matches!(declaration.kind,
          oxc::ast::ast::VariableDeclarationKind::Using
          | oxc::ast::ast::VariableDeclarationKind::AwaitUsing))
    });
    let resource_bindings: Vec<_> = if module_resources {
      let scopes = scoping.as_ref().unwrap();
      scopes
        .symbol_ids()
        .filter_map(|symbol| {
          let flags = scopes.symbol_flags(symbol);
          (scopes.symbol_scope_id(symbol) == scopes.root_scope_id()
            && flags.intersects(SymbolFlags::BlockScopedVariable | SymbolFlags::Class))
          .then(|| (CompactStr::from(scopes.symbol_name(symbol)), flags))
        })
        .collect()
    } else {
      Vec::new()
    };

    // Extract enum member values before the transformer converts enums.
    // This runs before Step 3 (transformer) because `optimize_const_enums` / `optimize_enums`
    // remove or rewrite enum declarations, making member values unrecoverable afterward.
    //
    // Both const and regular enums are extracted so member accesses can be inlined.
    // Tree-shaking in `include_statements.rs` skips including the enum declaration
    // when a member access will be inlined. Regular enum IIFEs are `@__PURE__`, so
    // they are naturally tree-shaken if no other references keep them alive.
    // Enums are a TypeScript-only construct, so only Ts/Tsx modules can declare
    // them. For plain JS/JSX modules, skip the full symbol-table walk and the map
    // allocation entirely (the result would always be empty).
    let enum_member_value_map = if matches!(parsed_type, OxcParseType::Ts | OxcParseType::Tsx) {
      let scoping_ref = scoping.as_mut().unwrap();
      let mut enum_values: FxHashMap<CompactStr, FxHashMap<CompactStr, ConstExportMeta>> =
        FxHashMap::default();

      // Walk enum declarations → body scopes → member bindings to collect values.
      for symbol_id in scoping_ref.symbol_ids() {
        let flags = scoping_ref.symbol_flags(symbol_id);
        if !(flags.is_const_enum() || flags.contains(SymbolFlags::RegularEnum)) {
          continue;
        }
        let Some(body_scopes) = scoping_ref.get_enum_body_scopes(symbol_id) else {
          continue;
        };
        let members = enum_values
          .entry(CompactStr::from(scoping_ref.symbol_name(symbol_id)))
          .or_default();

        for &body_scope in body_scopes {
          for (member_name, &member_sym) in scoping_ref.get_bindings(body_scope) {
            if let Some(value) = scoping_ref.get_enum_member_value(member_sym) {
              let rolldown_value = match value {
                oxc::syntax::constant_value::ConstantValue::Number(n) => ConstantValue::Number(*n),
                oxc::syntax::constant_value::ConstantValue::String(s) => {
                  ConstantValue::String(s.to_string())
                }
              };
              members.insert(
                CompactStr::from(member_name.as_str()),
                ConstExportMeta::new(rolldown_value, false),
              );
            }
          }
        }
      }
      enum_values
    } else {
      FxHashMap::default()
    };

    // Step 2: Run define plugin.
    if let Some(replace_global_define_config) = replace_global_define_config {
      ast.program.with_mut(
        |WithMutFields {
           program, allocator, ..
         }| {
          let ret = ReplaceGlobalDefines::new(allocator, replace_global_define_config.clone())
            .build(scoping.take().unwrap(), program);
          if !ret.changed {
            scoping = Some(ret.scoping);
          }
        },
      );
    }

    // Step 3: Transform TypeScript and jsx.
    // Note: Currently, the newest syntax oxc_transform can lower is ES2026
    // explicit resource management (`using`); the ES2025 regexp features are
    // not transformed. `LOWERABLE_ES_FEATURES` in `rolldown_common` holds the
    // full list `should_transform_js` is decided from.
    let is_not_js = !matches!(parsed_type, OxcParseType::Js);
    let mut preserve_jsx = false;
    if is_not_js
      || module_resources
      || bundle_options.transform_options.should_transform_js
      // Run transformer on JS files containing `</script` to handle tagged template literals.
      || contains_script_closing_tag(ast.source().as_bytes())
    {
      ast.program.with_mut(
        |WithMutFields {
           program, allocator, ..
         }| {
          // Pass file path only for non-JS modules (TS/TSX/JSX) to enable tsconfig discovery.
          // For plain JS files, we skip tsconfig lookup since they don't need TS-specific transformations.
          let mut transform_options = bundle_options
            .transform_options
            .options_for_file(is_not_js.then_some(Path::new(resolved_id)), &mut warnings)?;
          // Bundler module wrappers cannot retain a top-level resource lifetime.
          // Lower it before wrapping even when the output target supports using.
          if module_resources {
            std::sync::Arc::make_mut(&mut transform_options)
              .env
              .es2026
              .explicit_resource_management = true;
          }
          if !transform_options.jsx.jsx_plugin {
            preserve_jsx = true;
          }

          let scoping = self.recreate_scoping(&mut scoping, program);
          let ret = Transformer::new(allocator, Path::new(stable_id), &transform_options)
            .build_with_scoping(scoping, program);

          let (errors, transformer_warnings): (Vec<_>, Vec<_>) = ret
            .diagnostics
            .into_iter()
            .partition(|error| error.severity == OxcSeverity::Error);
          if !errors.is_empty() {
            return Err(BatchedBuildDiagnostic::from(
              BuildDiagnostic::from_oxc_diagnostics(
                errors,
                &source,
                resolved_id,
                Severity::Error,
                EventKind::TransformError,
              ),
            ));
          }
          warnings.extend(BuildDiagnostic::from_oxc_diagnostics(
            transformer_warnings,
            &source,
            resolved_id,
            Severity::Warning,
            EventKind::ToleratedTransform,
          ));
          Ok(())
        },
      )?;
    }

    debug_assert!(
      ast.program().source_type.is_javascript(),
      "ECMAScript transform must produce a JavaScript AST"
    );

    // Step 4: Run inject plugin.
    if !bundle_options.inject.is_empty() {
      ast.program.with_mut(
        |WithMutFields {
           program, allocator, ..
         }| {
          let new_scoping = self.recreate_scoping(&mut scoping, program);
          let inject_config = bundle_options.oxc_inject_global_variables_config.clone();
          let ret =
            InjectGlobalVariables::new(allocator, inject_config).build(new_scoping, program);
          if !ret.changed {
            scoping = Some(ret.scoping);
          }
        },
      );
    }

    // Step 5: Run DCE.
    // Avoid DCE for lazy export.
    if bundle_options.treeshake.is_some() && !has_lazy_export {
      ast.program.with_mut(
        |WithMutFields {
           program, allocator, ..
         }| {
      let mut scoping = self.recreate_scoping(&mut scoping, program);
      if fold_adjacent_define_property(program, &scoping, allocator) {
        scoping = self.recreate_scoping(&mut None, program);
      }
          Self::restore_resource_bindings(&mut scoping, &resource_bindings);
          // Oxc DCE assumes TDZ errors cannot occur. Keep lexical reads opaque until
          // the finalizer replaces this internal intrinsic with real binding guards.
          if super::temporal_reads::preserve_temporal_reads(program, &scoping, allocator) {
            scoping = self.recreate_scoping(&mut None, program);
          }
          let mut treeshake = TreeShakeOptions::from(&bundle_options.treeshake);
          treeshake.invalid_import_side_effects = true;
          let options = CompressOptions {
            target: bundle_options.transform_options.target.clone(),
            treeshake,
            ..CompressOptions::dce()
          };
          Compressor::new(allocator).dead_code_elimination_with_scoping(program, scoping, options);
        },
      );
    }

    // Step 6: Modify AST for Rolldown.
    let mut scoping = ast.program.with_mut(
      |WithMutFields {
         program, allocator, ..
       }| {
        let mut pre_processor = PreProcessor::new(
          allocator,
          bundle_options.keep_names,
          Some(&bundle_options.drop_labels),
        );
        pre_processor.visit_program(program);
        self.recreate_scoping(&mut None, program)
      },
    );
    Self::restore_resource_bindings(&mut scoping, &resource_bindings);

    Ok(ParseToEcmaAstResult {
      ast,
      scoping,
      has_lazy_export,
      warnings,
      preserve_jsx,
      enum_member_value_map,
    })
  }

  fn restore_resource_bindings(scoping: &mut Scoping, bindings: &[(CompactStr, SymbolFlags)]) {
    for (name, flags) in bindings {
      if let Some(symbol) = scoping.get_binding(scoping.root_scope_id(), name.as_str().into()) {
        *scoping.symbol_flags_mut(symbol) = *flags;
      }
    }
  }

  fn recreate_scoping(&mut self, scoping: &mut Option<Scoping>, program: &Program<'_>) -> Scoping {
    if let Some(scoping) = scoping.take() {
      return scoping;
    }
    let ret = semantic_builder_for_transform()
      // Preallocate memory for the underlying data structures.
      .with_stats(self.stats)
      .build(program)
      .semantic;
    self.stats = ret.stats();
    ret.into_scoping()
  }
}

fn fold_adjacent_define_property<'a>(
  program: &mut Program<'a>,
  scoping: &Scoping,
  _allocator: &'a oxc::allocator::Allocator,
) -> bool {
  if scoping
    .get_binding(scoping.root_scope_id(), "Object".into())
    .is_some()
  {
    return false;
  }
  let mut changed = false;
  let mut idx = 0;
  while idx + 1 < program.body.len() {
    let target_name = {
      let var_decl = match &program.body[idx] {
        Statement::VariableDeclaration(decl) => Some(decl.as_ref()),
        Statement::ExportDeclaration(export) => match &export.declaration {
          Declaration::VariableDeclaration(decl) => Some(decl.as_ref()),
          _ => None,
        },
        _ => None,
      };
      var_decl.and_then(|decl| {
        let last = decl.declarations.last()?;
        let oxc::ast::ast::BindingPattern::BindingIdentifier(ident) = &last.id else {
          return None;
        };
        let init = last.init.as_ref()?;
        let is_eligible_init = matches!(
          init,
          oxc::ast::ast::Expression::ObjectExpression(obj) if obj.properties.is_empty()
        ) || is_object_define_property_call(init);
        is_eligible_init.then(|| ident.name.as_str())
      })
    };
    let Some(target_name) = target_name else {
      idx += 1;
      continue;
    };
    let can_fold = match &program.body[idx + 1] {
      Statement::ExpressionStatement(expr_stmt) => match &expr_stmt.expression {
        oxc::ast::ast::Expression::CallExpression(call) => {
          is_object_define_property_callee(&call.callee)
            && call.arguments.len() >= 2
            && matches!(
              &call.arguments[0],
              oxc::ast::ast::Argument::Identifier(arg0) if arg0.name.as_str() == target_name
            )
            && call.arguments[1..]
              .iter()
              .all(is_safe_define_property_arg)
            && !call.arguments[1..]
              .iter()
              .any(|arg| argument_references_ident_eagerly(arg, target_name))
        }
        _ => false,
      },
      _ => false,
    };
    if !can_fold {
      idx += 1;
      continue;
    }

    let removed_stmt = program.body.remove(idx + 1);
    let Statement::ExpressionStatement(expr_stmt) = removed_stmt else {
      unreachable!();
    };
    let mut folded_call = expr_stmt.unbox().expression;
    let var_decl = match &mut program.body[idx] {
      Statement::VariableDeclaration(decl) => decl.as_mut(),
      Statement::ExportDeclaration(export) => match &mut export.declaration {
        Declaration::VariableDeclaration(decl) => decl.as_mut(),
        _ => unreachable!(),
      },
      _ => unreachable!(),
    };
    let last_decl = var_decl.declarations.last_mut().unwrap();
    if let Some(prev_init) = last_decl.init.take()
      && let oxc::ast::ast::Expression::CallExpression(call) = &mut folded_call
    {
      call.arguments[0] = oxc::ast::ast::Argument::from(prev_init);
    }
    last_decl.init = Some(folded_call);
    changed = true;
  }
  changed
}

fn is_object_define_property_callee(callee: &oxc::ast::ast::Expression<'_>) -> bool {
  let oxc::ast::ast::Expression::StaticMemberExpression(member) = callee else {
    return false;
  };
  if !matches!(
    member.property.name.as_str(),
    "defineProperty" | "defineProperties"
  ) {
    return false;
  }
  matches!(
    &member.object,
    oxc::ast::ast::Expression::Identifier(ident) if ident.name == "Object"
  )
}

fn is_object_define_property_call(expr: &oxc::ast::ast::Expression<'_>) -> bool {
  let oxc::ast::ast::Expression::CallExpression(call) = expr else {
    return false;
  };
  is_object_define_property_callee(&call.callee)
    && call.arguments.len() >= 2
    && call.arguments[1..].iter().all(is_safe_define_property_arg)
}

fn is_safe_define_property_arg(arg: &oxc::ast::ast::Argument<'_>) -> bool {
  let Some(expr) = arg.as_expression() else {
    return false;
  };
  match expr {
    oxc::ast::ast::Expression::StringLiteral(_)
    | oxc::ast::ast::Expression::NumericLiteral(_)
    | oxc::ast::ast::Expression::BooleanLiteral(_)
    | oxc::ast::ast::Expression::NullLiteral(_) => true,
    oxc::ast::ast::Expression::ObjectExpression(obj) => obj.properties.iter().all(|prop| {
      match prop {
        oxc::ast::ast::ObjectPropertyKind::ObjectProperty(p) => {
          !p.computed
            && matches!(p.kind, oxc::ast::ast::PropertyKind::Init)
            && matches!(
              &p.value,
              oxc::ast::ast::Expression::FunctionExpression(_)
                | oxc::ast::ast::Expression::ArrowFunctionExpression(_)
                | oxc::ast::ast::Expression::StringLiteral(_)
                | oxc::ast::ast::Expression::NumericLiteral(_)
                | oxc::ast::ast::Expression::BooleanLiteral(_)
                | oxc::ast::ast::Expression::NullLiteral(_)
            )
        }
        oxc::ast::ast::ObjectPropertyKind::SpreadProperty(_) => false,
      }
    }),
    _ => false,
  }
}

fn argument_references_ident_eagerly(
  arg: &oxc::ast::ast::Argument<'_>,
  ident_name: &str,
) -> bool {
  match arg.as_expression() {
    Some(expr) => expr_references_ident_eagerly(expr, ident_name),
    None => true,
  }
}

fn expr_references_ident_eagerly(
  expr: &oxc::ast::ast::Expression<'_>,
  ident_name: &str,
) -> bool {
  match expr {
    oxc::ast::ast::Expression::Identifier(ident) => ident.name.as_str() == ident_name,
    oxc::ast::ast::Expression::BooleanLiteral(_)
    | oxc::ast::ast::Expression::NullLiteral(_)
    | oxc::ast::ast::Expression::NumericLiteral(_)
    | oxc::ast::ast::Expression::BigIntLiteral(_)
    | oxc::ast::ast::Expression::RegExpLiteral(_)
    | oxc::ast::ast::Expression::StringLiteral(_)
    | oxc::ast::ast::Expression::FunctionExpression(_)
    | oxc::ast::ast::Expression::ArrowFunctionExpression(_) => false,
    oxc::ast::ast::Expression::ObjectExpression(obj) => obj.properties.iter().any(|prop| match prop {
      oxc::ast::ast::ObjectPropertyKind::ObjectProperty(p) => {
        !matches!(p.kind, oxc::ast::ast::PropertyKind::Init)
          || p.computed
          || expr_references_ident_eagerly(&p.value, ident_name)
      }
      oxc::ast::ast::ObjectPropertyKind::SpreadProperty(s) => {
        expr_references_ident_eagerly(&s.argument, ident_name)
      }
    }),
    _ => true,
  }
}

fn function_declaration_stmt_start(stmt: &Statement<'_>) -> Option<u32> {
  match stmt {
    Statement::FunctionDeclaration(decl) => Some(decl.span.start),
    Statement::ExportDeclaration(e) => match &e.declaration {
      Declaration::FunctionDeclaration(decl) => Some(decl.span.start),
      _ => None,
    },
    Statement::ExportDefaultDeclaration(e) => match &e.declaration {
      ExportDefaultDeclarationKind::FunctionDeclaration(decl) => Some(decl.span.start),
      _ => None,
    },
    _ => None,
  }
}

struct FunctionDeclarationStartMatcher {
  target_statement_starts: FxHashSet<u32>,
  matched_statement_starts: FxHashSet<u32>,
  remaining_target_count: usize,
}

impl FunctionDeclarationStartMatcher {
  fn new(target_statement_starts: FxHashSet<u32>) -> Self {
    let remaining_target_count = target_statement_starts.len();
    Self {
      target_statement_starts,
      matched_statement_starts: FxHashSet::default(),
      remaining_target_count,
    }
  }
}

impl<'ast> VisitJs<'ast> for FunctionDeclarationStartMatcher {
  fn visit_program(&mut self, program: &Program<'ast>) {
    for stmt in &program.body {
      if self.remaining_target_count == 0 {
        break;
      }
      self.visit_statement(stmt);
    }
  }

  fn visit_statement(&mut self, stmt: &Statement<'ast>) {
    if self.remaining_target_count == 0 {
      return;
    }
    if let Some(start) = function_declaration_stmt_start(stmt) {
      if self.target_statement_starts.contains(&start)
        && self.matched_statement_starts.insert(start)
      {
        self.remaining_target_count -= 1;
      }
    }
    if self.remaining_target_count > 0 {
      walk_js::walk_statement(self, stmt);
    }
  }
}

fn invalid_annotation_warnings(
  program: &Program<'_>,
  source: &ArcStr,
  resolved_id: &str,
) -> Vec<BuildDiagnostic> {
  let not_applied_comments: Vec<_> = program
    .comments
    .iter()
    .filter(|comment| {
      matches!(
        comment.content,
        CommentContent::PureNotApplied | CommentContent::NoSideEffectsNotApplied
      )
    })
    .collect();

  if not_applied_comments.is_empty() {
    return Vec::new();
  }

  let target_statement_starts: FxHashSet<u32> = not_applied_comments
    .iter()
    .filter(|comment| comment.content == CommentContent::PureNotApplied)
    .map(|comment| comment.attached_to)
    .collect();
  let mut function_declaration_start_matcher =
    FunctionDeclarationStartMatcher::new(target_statement_starts);
  function_declaration_start_matcher.visit_program(program);

  not_applied_comments
    .into_iter()
    .map(|comment| {
      let span = comment.span;
      let annotation = source[span.start as usize..span.end as usize].to_string();
      let is_before_function_declaration = comment.content == CommentContent::PureNotApplied
        && function_declaration_start_matcher
          .matched_statement_starts
          .contains(&comment.attached_to);
      BuildDiagnostic::invalid_annotation(
        resolved_id.to_string(),
        annotation,
        source.clone(),
        span,
        is_before_function_declaration,
        comment.content == CommentContent::NoSideEffectsNotApplied,
      )
      .with_severity_warning()
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use oxc::span::SourceType;
  use rolldown_ecmascript::EcmaCompiler;
  use rolldown_error::Severity;

  use super::invalid_annotation_warnings;

  #[test]
  fn invalid_annotations_have_warning_severity_and_matching_guidance() {
    let ast = EcmaCompiler::parse(
      "main.js",
      "/* #__PURE__ */ globalThis.foo; /* #__NO_SIDE_EFFECTS__ */ globalThis.bar;",
      SourceType::default(),
    )
    .unwrap();
    let source = ast.source().clone();
    let warnings = ast
      .program
      .with_dependent(|_owner, dep| invalid_annotation_warnings(&dep.program, &source, "main.js"));

    assert_eq!(warnings.len(), 2);
    for (warning, anchor) in warnings.iter().zip(["pure", "no-side-effects"]) {
      assert_eq!(warning.severity(), Severity::Warning);
      let rendered = warning.to_diagnostic().convert_to_string(false);
      assert!(rendered.contains(&format!(
        "Correct annotation placement: https://rolldown.rs/in-depth/dead-code-elimination#{anchor}"
      )));
    }
  }
}
