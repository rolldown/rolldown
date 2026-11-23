use std::cell::Cell;
use std::path::Path;

use arcstr::ArcStr;
use oxc::ast::ast::CommentContent;
use oxc::ast::ast::Program;
use oxc::ast::ast::{
  Argument, ArrowFunctionExpression, AssignmentExpression, AssignmentPattern,
  AssignmentTargetPropertyIdentifier, AssignmentTargetWithDefault, AwaitExpression,
  BindingIdentifier, BindingPattern, BlockStatement, CallExpression, CatchClause, ChainExpression,
  ConditionalExpression, Declaration, DoWhileStatement, ExportDefaultDeclarationKind,
  ExportSpecifier, Expression, ForInStatement, ForOfStatement, ForStatement, Function,
  IdentifierReference, IfStatement, LabeledStatement, LogicalExpression, LogicalOperator,
  NewExpression, ReturnStatement, Statement, StaticBlock, SwitchCase, TryStatement,
  VariableDeclaration, VariableDeclarationKind, VariableDeclarator, WhileStatement,
};
use oxc::ast_visit::{VisitJs, VisitJsMut, walk_js, walk_js_mut};
use oxc::diagnostics::{LabeledSpan, Severity as OxcSeverity};
use oxc::minifier::{CompressOptions, Compressor, TreeShakeOptions};
use oxc::semantic::{ReferenceId, ScopeFlags, ScopeId, Scoping, Stats, SymbolId};
use oxc::syntax::symbol::SymbolFlags;
use oxc::transformer::Transformer;
use oxc::transformer_plugins::{
  InjectGlobalVariables, ReplaceGlobalDefines, ReplaceGlobalDefinesConfig,
};
use oxc_str::CompactStr;

use rolldown_common::{ConstExportMeta, ConstantValue, ModuleId, NormalizedBundlerOptions};
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
    module_id: &ModuleId,
    should_warn_on_invalid_annotation: bool,
    parsed_type: &OxcParseType,
    replace_global_define_config: Option<&ReplaceGlobalDefinesConfig>,
    bundle_options: &NormalizedBundlerOptions,
    has_lazy_export: bool,
  ) -> BuildResult<ParseToEcmaAstResult> {
    let resolved_id = module_id.as_str();
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
      semantic_builder_for_transform().with_check_syntax_error(true).build(&dep.program)
    });

    let diagnostics = semantic_ret.diagnostics;
    if diagnostics.has_errors() {
      return Err(
        BuildDiagnostic::from_oxc_diagnostics(
          diagnostics.into_iter().filter(|d| d.severity == OxcSeverity::Error),
          &source,
          resolved_id,
          Severity::Error,
          EventKind::ParseError,
        )
        .into(),
      );
    }
    let mut warnings = BuildDiagnostic::from_oxc_diagnostics(
      diagnostics,
      &source,
      resolved_id,
      Severity::Warning,
      EventKind::ParseError,
    );
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
        let Some(body_scopes) = scoping_ref.get_enum_body_scopes(symbol_id) else { continue };
        let members =
          enum_values.entry(CompactStr::from(scoping_ref.symbol_name(symbol_id))).or_default();

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
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        let ret = ReplaceGlobalDefines::new(allocator, replace_global_define_config.clone())
          .build(scoping.take().unwrap(), program);
        if !ret.changed {
          scoping = Some(ret.scoping);
        }
      });
    }

    // `define` can insert a read of a binding or an annotated function, so this check runs after
    // Step 2. It runs before Step 3, which can add code.
    self.clear_unsafe_no_side_effects(&mut ast, &mut scoping);

    // Step 3: Transform TypeScript and jsx.
    // Note: Currently, the newest syntax oxc_transform can lower is ES2026
    // explicit resource management (`using`); the ES2025 regexp features are
    // not transformed. `LOWERABLE_ES_FEATURES` in `rolldown_common` holds the
    // full list `should_transform_js` is decided from.
    let is_not_js = !matches!(parsed_type, OxcParseType::Js);
    let mut preserve_jsx = false;
    if is_not_js
      || bundle_options.transform_options.should_transform_js
      // Run transformer on JS files containing `</script` to handle tagged template literals.
      || contains_script_closing_tag(ast.source().as_bytes())
    {
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        // Pass file path only for non-JS modules (TS/TSX/JSX) to enable tsconfig discovery.
        // For plain JS files, we skip tsconfig lookup since they don't need TS-specific transformations.
        let transform_options = bundle_options
          .transform_options
          .options_for_file(is_not_js.then_some(Path::new(module_id.raw_id())), &mut warnings)?;
        if !transform_options.jsx.jsx_plugin {
          preserve_jsx = true;
        }

        let scoping = self.recreate_scoping(&mut scoping, program);
        // Import attributes are not part of the file name, e.g. `_jsxFileName`.
        let stable_path =
          stable_id.strip_suffix(module_id.attributes_suffix()).unwrap_or(stable_id);
        let ret = Transformer::new(allocator, Path::new(stable_path), &transform_options)
          .build_with_scoping(scoping, program);

        let diagnostics = ret.diagnostics;
        if diagnostics.has_errors() {
          return Err(BatchedBuildDiagnostic::from(BuildDiagnostic::from_oxc_diagnostics(
            diagnostics.into_iter().filter(|d| d.severity == OxcSeverity::Error),
            &source,
            resolved_id,
            Severity::Error,
            EventKind::TransformError,
          )));
        }
        warnings.extend(BuildDiagnostic::from_oxc_diagnostics(
          diagnostics,
          &source,
          resolved_id,
          Severity::Warning,
          EventKind::ToleratedTransform,
        ));
        Ok(())
      })?;
    }

    debug_assert!(
      ast.program().source_type.is_javascript(),
      "ECMAScript transform must produce a JavaScript AST"
    );

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

    // Node treats a module with a top-level `await` as async, also when the `await` never runs.
    // Step 5 and `dropLabels` in Step 6 can remove such an `await` before the scan, so look for
    // one here. The walk reads the AST, not the source text, because `define` and the
    // `transform_ast` hook can insert an `await` that the text does not have.
    // See internal-docs/linking/module-execution-order/implementation.md.
    let runs_dce = bundle_options.treeshake.is_some() && !has_lazy_export;
    let source_has_top_level_await = (runs_dce || !bundle_options.drop_labels.is_empty())
      && TopLevelAwaitFinder::find(ast.program());

    // Step 5: Run DCE.
    // Avoid DCE for lazy export.
    if runs_dce {
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        let scoping = self.recreate_scoping(&mut scoping, program);
        let mut treeshake = TreeShakeOptions::from(&bundle_options.treeshake);
        treeshake.invalid_import_side_effects = true;
        let options = CompressOptions {
          target: bundle_options.transform_options.target.clone(),
          treeshake,
          ..CompressOptions::dce()
        };
        Compressor::new(allocator).dead_code_elimination_with_scoping(program, scoping, options);
      });
    }

    // Step 6: Modify AST for Rolldown.
    let (scoping, import_defer_spans) =
      ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
        let mut pre_processor = PreProcessor::new(
          allocator,
          bundle_options.keep_names,
          Some(&bundle_options.drop_labels),
        );
        pre_processor.visit_program(program);
        let defer_spans = pre_processor.take_defer_spans();
        (self.recreate_scoping(&mut None, program), defer_spans)
      });

    warnings.extend(import_defer_spans.into_iter().map(|span| {
      BuildDiagnostic::oxc_error(
        source.clone(),
        resolved_id.to_string(),
        String::new(),
        "`import defer` is currently lowered to a normal import. This changes execution timing because side effects run immediately instead of when the deferred import is first used.".to_string(),
        vec![LabeledSpan::at(
          span.start..span.end,
          "The deferred phase is removed here.",
        )],
        EventKind::UnsupportedFeatureError,
      )
      .with_severity_warning()
    }));

    Ok(ParseToEcmaAstResult {
      ast,
      scoping,
      has_lazy_export,
      warnings,
      preserve_jsx,
      enum_member_value_map,
      source_has_top_level_await,
    })
  }

  /// oxc treats every call of a `@__NO_SIDE_EFFECTS__` binding as pure. Drop the annotation of a
  /// binding whose call can still run code or throw, before the dead code elimination runs:
  /// - Direct `eval` can replace a `let`, a `var` or a function declaration without a write
  ///   reference.
  /// - Code that reads a `let`, `const` or `var` binding before its declaration, outside a nested
  ///   function, sees the binding before it is initialized. A call of that binding that runs only
  ///   under a condition gets a pure annotation, so that oxc can still remove it. See
  ///   `internal-docs/linking/module-execution-order/implementation.md`.
  fn clear_unsafe_no_side_effects(&mut self, ast: &mut EcmaAst, scoping: &mut Option<Scoping>) {
    // `define` drops the scoping when it changes the AST, and it can add an annotation. The next
    // pass uses the rebuilt scoping.
    let rebuilt = self.recreate_scoping(scoping, ast.program());
    let scoping_ref = &*scoping.insert(rebuilt);
    let no_side_effects = scoping_ref.no_side_effects();
    if no_side_effects.is_empty() {
      return;
    }
    let candidates = no_side_effects
      .iter()
      .copied()
      .filter(|&symbol_id| !scoping_ref.symbol_flags(symbol_id).is_function())
      .collect::<FxHashSet<_>>();
    let (mut symbols, guarded_calls) = if candidates.is_empty() {
      (FxHashSet::default(), vec![])
    } else {
      ReadBeforeDeclarationFinder::find(scoping_ref, candidates, ast.program())
    };
    let replaceable = if scoping_ref.root_scope_flags().contains_direct_eval() {
      no_side_effects
        .iter()
        .copied()
        .filter(|&symbol_id| {
          !scoping_ref.symbol_flags(symbol_id).is_const_variable()
            && scoping_ref
              .scope_flags(scoping_ref.symbol_scope_id(symbol_id))
              .contains_direct_eval()
        })
        .collect::<FxHashSet<_>>()
    } else {
      FxHashSet::default()
    };
    let pure_calls = guarded_calls
      .into_iter()
      .filter(|(_, symbol_id)| !replaceable.contains(symbol_id))
      .map(|(reference_id, _)| reference_id)
      .collect::<FxHashSet<_>>();
    symbols.extend(replaceable);
    if symbols.is_empty() {
      return;
    }
    ast.program.with_mut(|WithMutFields { program, .. }| {
      ClearNoSideEffects { symbols: &symbols, pure_calls: &pure_calls }.visit_program(program);
      *scoping = Some(self.recreate_scoping(&mut None, program));
    });
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

/// Finds a top-level `await`: an `await` expression, `for await` or `await using` outside a
/// function and a class static block. This is the scanner's rule for `TopLevelAwait`.
#[derive(Default)]
struct TopLevelAwaitFinder {
  found: bool,
}

impl TopLevelAwaitFinder {
  fn find(program: &Program<'_>) -> bool {
    let mut finder = Self::default();
    finder.visit_program(program);
    finder.found
  }
}

impl<'ast> VisitJs<'ast> for TopLevelAwaitFinder {
  fn visit_program(&mut self, program: &Program<'ast>) {
    for stmt in &program.body {
      self.visit_statement(stmt);
      if self.found {
        return;
      }
    }
  }

  fn visit_function(&mut self, _func: &Function<'ast>, _flags: ScopeFlags) {}

  fn visit_arrow_function_expression(&mut self, _func: &ArrowFunctionExpression<'ast>) {}

  fn visit_static_block(&mut self, _block: &StaticBlock<'ast>) {}

  fn visit_await_expression(&mut self, _it: &AwaitExpression<'ast>) {
    self.found = true;
  }

  fn visit_for_of_statement(&mut self, it: &ForOfStatement<'ast>) {
    if it.r#await {
      self.found = true;
    } else {
      walk_js::walk_for_of_statement(self, it);
    }
  }

  fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'ast>) {
    if it.kind == VariableDeclarationKind::AwaitUsing {
      self.found = true;
    } else {
      walk_js::walk_variable_declaration(self, it);
    }
  }
}

/// Finds the `@__NO_SIDE_EFFECTS__` bindings that code reads before their declaration, outside a
/// function nested in the declaring scope. Such a read sees the binding before it is initialized.
///
/// "Before" is AST order, not span order, because `define` gives the reads that it inserts an
/// empty span at 0. The walk skips a branch that a literal condition never runs, such as
/// `if (false) f();`, because that read never runs.
///
/// A read that runs only under another condition, such as in an `if` branch, a loop body, a
/// `catch` clause, after the calls that run first in a `try` block or after a top-level `return`,
/// does not count. The output can run the modules in another order than Node, so the condition can
/// be true in the output and false in Node. A declaration under a condition still counts as
/// reached.
struct ReadBeforeDeclarationFinder<'a> {
  scoping: &'a Scoping,
  /// The `let`, `const` and `var` bindings to check whose declaration the walk has not reached.
  pending: FxHashSet<SymbolId>,
  /// Whether a binding to check is declared below the root scope. If not, a read inside a
  /// function never counts, so the walk skips functions.
  visit_functions: bool,
  /// Each scope around the visited node, innermost last, and whether it is a function scope.
  scopes: Vec<(Option<ScopeId>, bool)>,
  /// Whether the visited node runs only under a condition.
  conditional: bool,
  /// Whether the walk passed a `return` at the top level of a CommonJS module. The code after it
  /// runs only when the `return` does not run.
  returned: bool,
  /// The callees of the calls that run first in a `try` block. They count in a conditional walk.
  /// A call in the first argument of another call counts only when the outer callee is a function
  /// declaration, because the read of another callee can throw before the argument runs.
  leading_reads: FxHashSet<ReferenceId>,
  found: FxHashSet<SymbolId>,
  /// The callee and the binding of each call or `new` that runs only under a condition before the
  /// declaration of the binding.
  guarded_calls: Vec<(ReferenceId, SymbolId)>,
}

impl ReadBeforeDeclarationFinder<'_> {
  /// Returns the bindings that code reads before their declaration, and the callee and the binding
  /// of each call of those bindings that runs only under a condition before the declaration.
  /// Clearing the annotation of a binding affects all of its calls, so oxc removes such a call only
  /// when it gets its own pure annotation.
  fn find(
    scoping: &Scoping,
    candidates: FxHashSet<SymbolId>,
    program: &Program<'_>,
  ) -> (FxHashSet<SymbolId>, Vec<(ReferenceId, SymbolId)>) {
    let root_scope_id = scoping.root_scope_id();
    let visit_functions =
      candidates.iter().any(|&symbol_id| scoping.symbol_scope_id(symbol_id) != root_scope_id);
    let mut finder = ReadBeforeDeclarationFinder {
      scoping,
      pending: candidates,
      visit_functions,
      scopes: vec![],
      conditional: false,
      returned: false,
      leading_reads: FxHashSet::default(),
      found: FxHashSet::default(),
      guarded_calls: vec![],
    };
    finder.visit_program(program);
    let mut guarded_calls = finder.guarded_calls;
    guarded_calls.retain(|(_, symbol_id)| finder.found.contains(symbol_id));
    (finder.found, guarded_calls)
  }

  fn conditionally(&mut self, visit: impl FnOnce(&mut Self)) {
    let conditional = std::mem::replace(&mut self.conditional, true);
    visit(self);
    self.conditional = conditional;
  }

  fn record_guarded_call(&mut self, callee: &Expression<'_>) {
    if self.is_conditional()
      && let Expression::Identifier(ident) = callee.without_parentheses()
      && let Some(reference_id) = ident.reference_id.get()
      && !self.leading_reads.contains(&reference_id)
      && let Some(symbol_id) = self.early_read(ident)
    {
      self.guarded_calls.push((reference_id, symbol_id));
    }
  }

  fn is_conditional(&self) -> bool {
    self.conditional || self.returned
  }

  fn in_function(&self) -> bool {
    self.scopes.iter().any(|&(_, is_function)| is_function)
  }

  /// The binding to check that `ident` reads before its declaration, outside a function nested in
  /// the declaring scope.
  fn early_read(&self, ident: &IdentifierReference<'_>) -> Option<SymbolId> {
    let symbol_id = self.scoping.get_reference(ident.reference_id.get()?).symbol_id()?;
    if !self.pending.contains(&symbol_id) {
      return None;
    }
    let declaring_scope = Some(self.scoping.symbol_scope_id(symbol_id));
    let in_nested_function = self
      .scopes
      .iter()
      .rev()
      .take_while(|(scope_id, _)| *scope_id != declaring_scope)
      .any(|&(_, is_function)| is_function);
    (!in_nested_function).then_some(symbol_id)
  }
}

/// The truthiness of a literal condition, such as `false` or `0`. `None` for other expressions.
fn literal_truthiness(expr: &Expression<'_>) -> Option<bool> {
  match expr.without_parentheses() {
    Expression::BooleanLiteral(lit) => Some(lit.value),
    Expression::NullLiteral(_) => Some(false),
    // A numeric literal is never negative or `NaN`.
    Expression::NumericLiteral(lit) => Some(lit.value > 0.0),
    Expression::StringLiteral(lit) => Some(!lit.value.is_empty()),
    // `value` is in base 10, so `0x0n` is zero too.
    Expression::BigIntLiteral(lit) => Some(!lit.is_zero()),
    _ => None,
  }
}

/// Whether a literal is `null`, for the left operand of `??`. `None` for other expressions.
fn literal_is_nullish(expr: &Expression<'_>) -> Option<bool> {
  match expr.without_parentheses() {
    Expression::NullLiteral(_) => Some(true),
    Expression::BooleanLiteral(_)
    | Expression::NumericLiteral(_)
    | Expression::StringLiteral(_)
    | Expression::BigIntLiteral(_) => Some(false),
    _ => None,
  }
}

/// Whether the right operand of `it` runs, when its left operand is a literal. `None` for other
/// left operands.
fn logical_right_runs(it: &LogicalExpression<'_>) -> Option<bool> {
  match it.operator {
    LogicalOperator::And => literal_truthiness(&it.left),
    LogicalOperator::Or => literal_truthiness(&it.left).map(|value| !value),
    LogicalOperator::Coalesce => literal_is_nullish(&it.left),
  }
}

/// The calls that run in `block` before any other code of the block that can throw. In
/// `try { noop(f()); g(); }`, the code reads `noop`, runs `f()` and then runs `noop(f())`. `g()`
/// runs only when they do not throw, and a `catch` clause can recover from that error. A call
/// with a plain-name callee reads the name before it runs the first argument. That read can throw:
/// a binding can be uninitialized, and a global can be a getter. The output can run the modules in
/// another order than Node, so the read can throw in Node and not in the output. The walk goes into
/// the first argument only when `callee_read_is_safe` proves that the read does not throw. The
/// first statement can also be a declaration, whose first initializer runs first, or an `if`,
/// whose test runs first.
pub fn leading_calls<'b, 'a>(
  block: &'b BlockStatement<'a>,
  callee_read_is_safe: impl Fn(&IdentifierReference<'a>) -> bool,
) -> Vec<&'b CallExpression<'a>> {
  let mut calls = vec![];
  let mut stmt = block.body.first();
  let expr = loop {
    match stmt {
      Some(Statement::ExpressionStatement(it)) => break &it.expression,
      Some(Statement::IfStatement(it)) => break &it.test,
      Some(Statement::VariableDeclaration(it)) => {
        match it.declarations.first().and_then(|declarator| declarator.init.as_ref()) {
          Some(init) => break init,
          None => return calls,
        }
      }
      Some(Statement::BlockStatement(it)) => stmt = it.body.first(),
      Some(Statement::TryStatement(it)) => stmt = it.block.body.first(),
      _ => return calls,
    }
  };
  let mut expr = expr;
  loop {
    expr = match expr.without_parentheses() {
      Expression::SequenceExpression(it) => {
        let Some(first) = it.expressions.first() else { break };
        first
      }
      Expression::ConditionalExpression(it) => match literal_truthiness(&it.test) {
        Some(true) => &it.consequent,
        Some(false) => &it.alternate,
        None => &it.test,
      },
      Expression::LogicalExpression(it) => match logical_right_runs(it) {
        Some(true) => &it.right,
        Some(false) => break,
        None => &it.left,
      },
      Expression::CallExpression(call) => {
        calls.push(&**call);
        if !matches!(
          call.callee.without_parentheses(),
          Expression::Identifier(ident) if callee_read_is_safe(ident)
        ) {
          break;
        }
        let Some(argument) = call.arguments.first().and_then(Argument::as_expression) else {
          break;
        };
        argument
      }
      _ => break,
    };
  }
  calls
}

impl<'ast> VisitJs<'ast> for ReadBeforeDeclarationFinder<'_> {
  fn visit_program(&mut self, program: &Program<'ast>) {
    self.scopes.push((program.scope_id.get(), false));
    for stmt in &program.body {
      if self.pending.is_empty() {
        break;
      }
      self.visit_statement(stmt);
    }
  }

  fn visit_binding_identifier(&mut self, ident: &BindingIdentifier<'ast>) {
    if let Some(symbol_id) = ident.symbol_id.get() {
      self.pending.remove(&symbol_id);
    }
  }

  fn visit_if_statement(&mut self, it: &IfStatement<'ast>) {
    self.visit_expression(&it.test);
    let Some(value) = literal_truthiness(&it.test) else {
      self.conditionally(|this| {
        this.visit_statement(&it.consequent);
        if let Some(alternate) = &it.alternate {
          this.visit_statement(alternate);
        }
      });
      return;
    };
    if value {
      self.visit_statement(&it.consequent);
    } else if let Some(alternate) = &it.alternate {
      self.visit_statement(alternate);
    }
  }

  fn visit_conditional_expression(&mut self, it: &ConditionalExpression<'ast>) {
    self.visit_expression(&it.test);
    let Some(value) = literal_truthiness(&it.test) else {
      self.conditionally(|this| {
        this.visit_expression(&it.consequent);
        this.visit_expression(&it.alternate);
      });
      return;
    };
    self.visit_expression(if value { &it.consequent } else { &it.alternate });
  }

  fn visit_logical_expression(&mut self, it: &LogicalExpression<'ast>) {
    self.visit_expression(&it.left);
    match logical_right_runs(it) {
      Some(true) => self.visit_expression(&it.right),
      Some(false) => {}
      None => self.conditionally(|this| this.visit_expression(&it.right)),
    }
  }

  fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'ast>) {
    if it.operator.is_logical() {
      self.visit_assignment_target(&it.left);
      self.conditionally(|this| this.visit_expression(&it.right));
    } else {
      walk_js::walk_assignment_expression(self, it);
    }
  }

  fn visit_for_statement(&mut self, it: &ForStatement<'ast>) {
    self.enter_scope(ScopeFlags::empty(), &it.scope_id);
    if let Some(init) = &it.init {
      self.visit_for_statement_init(init);
    }
    if let Some(test) = &it.test {
      self.visit_expression(test);
    }
    self.conditionally(|this| {
      if let Some(update) = &it.update {
        this.visit_expression(update);
      }
      this.visit_statement(&it.body);
    });
    self.leave_scope();
  }

  fn visit_for_in_statement(&mut self, it: &ForInStatement<'ast>) {
    self.enter_scope(ScopeFlags::empty(), &it.scope_id);
    self.conditionally(|this| this.visit_for_statement_left(&it.left));
    self.visit_expression(&it.right);
    self.conditionally(|this| this.visit_statement(&it.body));
    self.leave_scope();
  }

  fn visit_for_of_statement(&mut self, it: &ForOfStatement<'ast>) {
    self.enter_scope(ScopeFlags::empty(), &it.scope_id);
    self.conditionally(|this| this.visit_for_statement_left(&it.left));
    self.visit_expression(&it.right);
    self.conditionally(|this| this.visit_statement(&it.body));
    self.leave_scope();
  }

  fn visit_while_statement(&mut self, it: &WhileStatement<'ast>) {
    self.visit_expression(&it.test);
    self.conditionally(|this| this.visit_statement(&it.body));
  }

  // `break` or `continue` can skip the rest of the body and the test.
  fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'ast>) {
    self.conditionally(|this| walk_js::walk_do_while_statement(this, it));
  }

  fn visit_switch_case(&mut self, it: &SwitchCase<'ast>) {
    self.conditionally(|this| walk_js::walk_switch_case(this, it));
  }

  fn visit_catch_clause(&mut self, it: &CatchClause<'ast>) {
    self.conditionally(|this| walk_js::walk_catch_clause(this, it));
  }

  // A `catch` clause can recover from an error that skips the rest of the `try` block.
  fn visit_try_statement(&mut self, it: &TryStatement<'ast>) {
    let Some(handler) = &it.handler else {
      walk_js::walk_try_statement(self, it);
      return;
    };
    if !self.is_conditional() {
      // A function declaration is initialized before any code runs. This pass cannot prove that
      // an import or a global is safe to read.
      let scoping = self.scoping;
      let callee_read_is_safe = |ident: &IdentifierReference<'ast>| {
        ident
          .reference_id
          .get()
          .and_then(|reference_id| scoping.get_reference(reference_id).symbol_id())
          .is_some_and(|symbol_id| scoping.symbol_flags(symbol_id).is_function())
      };
      self.leading_reads.extend(
        leading_calls(&it.block, callee_read_is_safe).into_iter().filter_map(|call| {
          match call.callee.without_parentheses() {
            Expression::Identifier(ident) => ident.reference_id.get(),
            _ => None,
          }
        }),
      );
    }
    self.conditionally(|this| {
      this.visit_block_statement(&it.block);
      this.visit_catch_clause(handler);
    });
    if let Some(finalizer) = &it.finalizer {
      self.visit_block_statement(finalizer);
    }
  }

  // `break` can skip the rest of a labeled statement.
  fn visit_labeled_statement(&mut self, it: &LabeledStatement<'ast>) {
    self.conditionally(|this| walk_js::walk_labeled_statement(this, it));
  }

  fn visit_chain_expression(&mut self, it: &ChainExpression<'ast>) {
    self.conditionally(|this| walk_js::walk_chain_expression(this, it));
  }

  fn visit_assignment_pattern(&mut self, it: &AssignmentPattern<'ast>) {
    self.visit_binding_pattern(&it.left);
    self.conditionally(|this| this.visit_expression(&it.right));
  }

  fn visit_assignment_target_with_default(&mut self, it: &AssignmentTargetWithDefault<'ast>) {
    self.visit_assignment_target(&it.binding);
    self.conditionally(|this| this.visit_expression(&it.init));
  }

  fn visit_assignment_target_property_identifier(
    &mut self,
    it: &AssignmentTargetPropertyIdentifier<'ast>,
  ) {
    self.visit_identifier_reference(&it.binding);
    if let Some(init) = &it.init {
      self.conditionally(|this| this.visit_expression(init));
    }
  }

  fn enter_scope(&mut self, flags: ScopeFlags, scope_id: &Cell<Option<ScopeId>>) {
    self.scopes.push((scope_id.get(), flags.is_function()));
  }

  fn leave_scope(&mut self) {
    self.scopes.pop();
  }

  fn visit_function(&mut self, func: &Function<'ast>, flags: ScopeFlags) {
    if self.visit_functions {
      walk_js::walk_function(self, func, flags);
    }
  }

  fn visit_arrow_function_expression(&mut self, func: &ArrowFunctionExpression<'ast>) {
    if self.visit_functions {
      walk_js::walk_arrow_function_expression(self, func);
    }
  }

  // `export { fn }` does not read the binding.
  fn visit_export_specifier(&mut self, _it: &ExportSpecifier<'ast>) {}

  fn visit_return_statement(&mut self, it: &ReturnStatement<'ast>) {
    walk_js::walk_return_statement(self, it);
    if !self.in_function() {
      self.returned = true;
    }
  }

  fn visit_call_expression(&mut self, it: &CallExpression<'ast>) {
    self.record_guarded_call(&it.callee);
    walk_js::walk_call_expression(self, it);
  }

  // oxc applies the annotation to `new` too.
  fn visit_new_expression(&mut self, it: &NewExpression<'ast>) {
    self.record_guarded_call(&it.callee);
    walk_js::walk_new_expression(self, it);
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference<'ast>) {
    if self.is_conditional()
      && !ident.reference_id.get().is_some_and(|id| self.leading_reads.contains(&id))
    {
      return;
    }
    if let Some(symbol_id) = self.early_read(ident) {
      self.found.insert(symbol_id);
    }
  }
}

/// Drops `@__NO_SIDE_EFFECTS__` from the functions that initialize `symbols`, and marks the calls
/// and `new` expressions whose callee is in `pure_calls` as pure.
struct ClearNoSideEffects<'a> {
  symbols: &'a FxHashSet<SymbolId>,
  pure_calls: &'a FxHashSet<ReferenceId>,
}

impl<'ast> VisitJsMut<'ast> for ClearNoSideEffects<'_> {
  fn visit_call_expression(&mut self, call: &mut CallExpression<'ast>) {
    if let Expression::Identifier(ident) = call.callee.without_parentheses()
      && ident.reference_id.get().is_some_and(|id| self.pure_calls.contains(&id))
    {
      call.pure = true;
    }
    walk_js_mut::walk_call_expression(self, call);
  }

  fn visit_new_expression(&mut self, new_expr: &mut NewExpression<'ast>) {
    if let Expression::Identifier(ident) = new_expr.callee.without_parentheses()
      && ident.reference_id.get().is_some_and(|id| self.pure_calls.contains(&id))
    {
      new_expr.pure = true;
    }
    walk_js_mut::walk_new_expression(self, new_expr);
  }

  fn visit_function(&mut self, func: &mut Function<'ast>, flags: ScopeFlags) {
    if func.id.as_ref().is_some_and(|id| self.symbols.contains(&id.symbol_id())) {
      func.pure = false;
    }
    walk_js_mut::walk_function(self, func, flags);
  }

  fn visit_variable_declarator(&mut self, decl: &mut VariableDeclarator<'ast>) {
    if let BindingPattern::BindingIdentifier(id) = &decl.id
      && self.symbols.contains(&id.symbol_id())
    {
      match &mut decl.init {
        Some(Expression::FunctionExpression(func)) => func.pure = false,
        Some(Expression::ArrowFunctionExpression(func)) => func.pure = false,
        _ => {}
      }
    }
    walk_js_mut::walk_variable_declarator(self, decl);
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

  use super::{TopLevelAwaitFinder, invalid_annotation_warnings};

  #[test]
  fn top_level_await_finder_skips_functions() {
    for (source, expected) in [
      ("if (false) await 0;", true),
      ("for await (const x of []) {}", true),
      ("{ await using r = null; }", true),
      ("class A { [await 0]() {} }", true),
      ("async function f() { await 0; } const g = async () => { await 0; };", false),
      ("class A { async m() { await 0; } static { } }", false),
      ("for (const x of []) {} using r = null;", false),
    ] {
      let ast = EcmaCompiler::parse("main.js", source, SourceType::mjs()).unwrap();
      let found = ast.program.with_dependent(|_owner, dep| TopLevelAwaitFinder::find(&dep.program));
      assert_eq!(found, expected, "{source}");
    }
  }

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
