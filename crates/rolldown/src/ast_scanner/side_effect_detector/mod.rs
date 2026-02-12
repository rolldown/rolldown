#[cfg(test)]
use oxc::ast::ast::ChainElement;
use oxc::{
  allocator::{Address, GetAddress, UnstableAddress},
  ast::ast::{
    AssignmentTarget, CallExpression, Expression, IdentifierReference, NewExpression, Statement,
  },
  ast_visit::{Visit, walk},
};
use oxc_ecmascript::{
  GlobalContext,
  side_effects::{
    MayHaveSideEffects, MayHaveSideEffectsContext,
    PropertyReadSideEffects as OxcPropertyReadSideEffects, is_pure_function,
  },
};
use rolldown_common::{AstScopes, FlatOptions, SharedNormalizedBundlerOptions, SideEffectDetail};
use rustc_hash::FxHashSet;

/// Detect if a statement "may" have side effect.
pub struct SideEffectDetector<'a> {
  pub scope: &'a AstScopes,
  options: &'a SharedNormalizedBundlerOptions,
  flat_options: FlatOptions,
  manual_pure_functions: Option<Vec<String>>,
  /// This field is only used for `LinkStage#cross_module_optimization`.
  side_effect_free_function_symbol_ref: Option<&'a FxHashSet<Address>>,
}

impl<'a> SideEffectDetector<'a> {
  pub fn new(
    scope: &'a AstScopes,
    flat_options: FlatOptions,
    options: &'a SharedNormalizedBundlerOptions,
    side_effect_free_function_symbol_ref: Option<&'a FxHashSet<Address>>,
  ) -> Self {
    let manual_pure_functions =
      options.treeshake.manual_pure_functions().map(|set| set.iter().cloned().collect());
    Self {
      scope,
      options,
      flat_options,
      manual_pure_functions,
      side_effect_free_function_symbol_ref,
    }
  }

  #[inline]
  fn is_unresolved_reference(&self, ident_ref: &IdentifierReference<'_>) -> bool {
    ident_ref.reference_id.get().is_some_and(|reference_id| self.scope.is_unresolved(reference_id))
  }

  fn analyze_statement(&self, stmt: &Statement<'_>) -> AnalysisResult {
    let scan = ScanState::from_statement(
      stmt,
      self.scope,
      self.flat_options,
      self.side_effect_free_function_symbol_ref,
    );
    let has_side_effects = {
      let ctx = DetectorContext {
        detector: self,
        side_effect_free_callee_addresses: &scan.side_effect_free_callee_addresses,
      };
      stmt.may_have_side_effects(&ctx)
    };
    AnalysisResult {
      has_side_effects,
      has_global_var_access: scan.has_global_var_access,
      has_pure_annotation: scan.has_pure_annotation,
    }
  }

  fn analyze_expression(&self, expr: &Expression<'_>) -> AnalysisResult {
    let scan = ScanState::from_expression(
      expr,
      self.scope,
      self.flat_options,
      self.side_effect_free_function_symbol_ref,
    );
    let has_side_effects = {
      let ctx = DetectorContext {
        detector: self,
        side_effect_free_callee_addresses: &scan.side_effect_free_callee_addresses,
      };
      expr.may_have_side_effects(&ctx)
    };
    AnalysisResult {
      has_side_effects,
      has_global_var_access: scan.has_global_var_access,
      has_pure_annotation: scan.has_pure_annotation,
    }
  }

  #[cfg(test)]
  fn extract_first_part_of_member_expr_like(expr: &'a Expression) -> Option<&'a str> {
    let mut cur = expr;
    loop {
      match cur {
        Expression::Identifier(ident) => break Some(ident.name.as_str()),
        Expression::ComputedMemberExpression(expr) => {
          cur = &expr.object;
        }
        Expression::StaticMemberExpression(expr) => {
          cur = &expr.object;
        }
        Expression::CallExpression(expr) => {
          cur = &expr.callee;
        }
        Expression::ChainExpression(expr) => match expr.expression {
          ChainElement::CallExpression(ref call_expression) => {
            cur = &call_expression.callee;
          }
          ChainElement::ComputedMemberExpression(ref computed_member_expression) => {
            cur = &computed_member_expression.object;
          }
          ChainElement::StaticMemberExpression(ref static_member_expression) => {
            cur = &static_member_expression.object;
          }
          ChainElement::TSNonNullExpression(_) | ChainElement::PrivateFieldExpression(_) => {
            break None;
          }
        },
        _ => break None,
      }
    }
  }

  fn get_pure_cjs_assignment_rhs<'ast>(
    &self,
    stmt: &'ast Statement<'ast>,
  ) -> Option<&'ast Expression<'ast>> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
      return None;
    };
    let Expression::AssignmentExpression(assign_expr) = expr_stmt.expression.without_parentheses()
    else {
      return None;
    };

    let member_expr = match &assign_expr.left {
      AssignmentTarget::ComputedMemberExpression(_)
      | AssignmentTarget::StaticMemberExpression(_) => assign_expr.left.to_member_expression(),
      _ => return None,
    };

    let Expression::Identifier(object_ident) = member_expr.object().without_parentheses() else {
      return None;
    };

    (object_ident.name == "exports"
      && self.is_unresolved_reference(object_ident)
      && member_expr.static_property_name().is_some())
    .then_some(&assign_expr.right)
  }

  pub fn detect_side_effect_of_stmt(&self, stmt: &Statement<'_>) -> SideEffectDetail {
    if let Some(rhs_expr) = self.get_pure_cjs_assignment_rhs(stmt) {
      let rhs_analysis = self.analyze_expression(rhs_expr);
      let mut detail = SideEffectDetail::PureCjs;
      detail.set(SideEffectDetail::Unknown, rhs_analysis.has_side_effects);
      detail.set(SideEffectDetail::GlobalVarAccess, rhs_analysis.has_global_var_access);
      detail.set(SideEffectDetail::PureAnnotation, rhs_analysis.has_pure_annotation);
      return detail;
    }

    let analysis = self.analyze_statement(stmt);
    let mut detail = SideEffectDetail::empty();
    detail.set(SideEffectDetail::Unknown, analysis.has_side_effects);
    detail.set(SideEffectDetail::GlobalVarAccess, analysis.has_global_var_access);
    detail.set(SideEffectDetail::PureAnnotation, analysis.has_pure_annotation);
    detail
  }
}

#[derive(Debug, Clone, Copy, Default)]
struct AnalysisResult {
  has_side_effects: bool,
  has_global_var_access: bool,
  has_pure_annotation: bool,
}

struct DetectorContext<'d, 'a> {
  detector: &'d SideEffectDetector<'a>,
  side_effect_free_callee_addresses: &'d FxHashSet<Address>,
}

impl<'a> GlobalContext<'a> for DetectorContext<'_, '_> {
  fn is_global_reference(&self, reference: &IdentifierReference<'a>) -> bool {
    reference
      .reference_id
      .get()
      .is_some_and(|reference_id| self.detector.scope.is_unresolved(reference_id))
  }
}

impl MayHaveSideEffectsContext<'_> for DetectorContext<'_, '_> {
  fn annotations(&self) -> bool {
    !self.detector.flat_options.ignore_annotations()
  }

  fn manual_pure_functions(&self, callee: &Expression) -> bool {
    if self.side_effect_free_callee_addresses.contains(&callee.address()) {
      return true;
    }

    self
      .detector
      .manual_pure_functions
      .as_ref()
      .is_some_and(|manual_pure_functions| is_pure_function(callee, manual_pure_functions))
  }

  fn property_read_side_effects(&self) -> OxcPropertyReadSideEffects {
    if self.detector.flat_options.property_read_side_effects() {
      OxcPropertyReadSideEffects::All
    } else {
      OxcPropertyReadSideEffects::None
    }
  }

  fn unknown_global_side_effects(&self) -> bool {
    self.detector.options.treeshake.unknown_global_side_effects()
  }
}

struct ScanState<'a> {
  scope: &'a AstScopes,
  ignore_annotations: bool,
  side_effect_free_call_expr_addresses: Option<&'a FxHashSet<Address>>,
  has_global_var_access: bool,
  has_pure_annotation: bool,
  side_effect_free_callee_addresses: FxHashSet<Address>,
}

impl<'a> ScanState<'a> {
  fn from_statement<'ast>(
    stmt: &Statement<'ast>,
    scope: &'a AstScopes,
    flat_options: FlatOptions,
    side_effect_free_call_expr_addresses: Option<&'a FxHashSet<Address>>,
  ) -> Self {
    let mut scan = Self {
      scope,
      ignore_annotations: flat_options.ignore_annotations(),
      side_effect_free_call_expr_addresses,
      has_global_var_access: false,
      has_pure_annotation: false,
      side_effect_free_callee_addresses: FxHashSet::default(),
    };
    scan.visit_statement(stmt);
    scan
  }

  fn from_expression<'ast>(
    expr: &Expression<'ast>,
    scope: &'a AstScopes,
    flat_options: FlatOptions,
    side_effect_free_call_expr_addresses: Option<&'a FxHashSet<Address>>,
  ) -> Self {
    let mut scan = Self {
      scope,
      ignore_annotations: flat_options.ignore_annotations(),
      side_effect_free_call_expr_addresses,
      has_global_var_access: false,
      has_pure_annotation: false,
      side_effect_free_callee_addresses: FxHashSet::default(),
    };
    scan.visit_expression(expr);
    scan
  }
}

impl<'ast> Visit<'ast> for ScanState<'_> {
  fn visit_function(
    &mut self,
    _it: &oxc::ast::ast::Function<'ast>,
    _flags: oxc::semantic::ScopeFlags,
  ) {
    // Function bodies are not evaluated at declaration time.
  }

  fn visit_arrow_function_expression(
    &mut self,
    _it: &oxc::ast::ast::ArrowFunctionExpression<'ast>,
  ) {
    // Function bodies are not evaluated at declaration time.
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference<'ast>) {
    if ident.reference_id.get().is_some_and(|reference_id| self.scope.is_unresolved(reference_id)) {
      self.has_global_var_access = true;
    }
  }

  fn visit_call_expression(&mut self, expr: &CallExpression<'ast>) {
    let is_side_effect_free_call = self
      .side_effect_free_call_expr_addresses
      .is_some_and(|set| set.contains(&expr.unstable_address()));

    if !self.ignore_annotations && (expr.pure || is_side_effect_free_call) {
      self.has_pure_annotation = true;
    }

    if is_side_effect_free_call {
      self.side_effect_free_callee_addresses.insert(expr.callee.address());
    }

    walk::walk_call_expression(self, expr);
  }

  fn visit_new_expression(&mut self, expr: &NewExpression<'ast>) {
    if !self.ignore_annotations && expr.pure {
      self.has_pure_annotation = true;
    }

    walk::walk_new_expression(self, expr);
  }
}
#[cfg(test)]
mod test {
  use std::sync::Arc;

  use itertools::Itertools;
  use oxc::{parser::Parser, span::SourceType};
  use rolldown_common::{AstScopes, NormalizedBundlerOptions, SideEffectDetail};
  use rolldown_ecmascript::{EcmaAst, EcmaCompiler};

  use super::SideEffectDetector;
  use rolldown_common::FlatOptions;

  fn get_statements_side_effect(code: &str) -> bool {
    let source_type = SourceType::tsx();
    let ast = EcmaCompiler::parse("<Noop>", code, source_type).unwrap();
    let semantic = EcmaAst::make_semantic(ast.program(), false);
    let scoping = semantic.into_scoping();
    let ast_scopes = AstScopes::new(scoping);

    let options = Arc::new(NormalizedBundlerOptions::default());
    let flags = FlatOptions::from_shared_options(&options);
    ast.program().body.iter().any(|stmt| {
      SideEffectDetector::new(&ast_scopes, flags, &options, None)
        .detect_side_effect_of_stmt(stmt)
        .has_side_effect()
    })
  }

  fn get_statements_side_effect_details(code: &str) -> Vec<SideEffectDetail> {
    let source_type = SourceType::tsx();
    let ast = EcmaCompiler::parse("<Noop>", code, source_type).unwrap();
    let semantic = EcmaAst::make_semantic(ast.program(), false);
    let scoping = semantic.into_scoping();
    let ast_scopes = AstScopes::new(scoping);

    let options = Arc::new(NormalizedBundlerOptions::default());
    let flags = FlatOptions::from_shared_options(&options);
    ast
      .program()
      .body
      .iter()
      .map(|stmt| {
        SideEffectDetector::new(&ast_scopes, flags, &options, None).detect_side_effect_of_stmt(stmt)
      })
      .collect_vec()
  }

  #[test]
  fn test_side_effect() {
    assert!(get_statements_side_effect("export { a }"));
    assert!(!get_statements_side_effect("const a = {}"));
    assert!(!get_statements_side_effect(
      "const PatchFlags = {
        'TEXT':1,
        '1':'TEXT',
        'CLASS':2,
        '2':'CLASS',
        'STYLE':4,
        '4':'STYLE',
        'PROPS':8,
        '8':'PROPS',
        'FULL_PROPS':16,
        '16':'FULL_PROPS',
        'NEED_HYDRATION':32,
        '32':'NEED_HYDRATION',
        'STABLE_FRAGMENT':64,
        '64':'STABLE_FRAGMENT',
        'KEYED_FRAGMENT':128,
        '128':'KEYED_FRAGMENT',
        'UNKEYED_FRAGMENT':256,
        '256':'UNKEYED_FRAGMENT',
        'NEED_PATCH':512,
        '512':'NEED_PATCH',
        'DYNAMIC_SLOTS':1024,
        '1024':'DYNAMIC_SLOTS',
        'DEV_ROOT_FRAGMENT':2048,
        '2048':'DEV_ROOT_FRAGMENT',
        'HOISTED': -1,
        '-1':'HOISTED',
        'BAIL': -2,
        '-2':'BAIL'
      };",
    ));
  }

  #[test]
  fn test_template_literal() {
    assert!(!get_statements_side_effect("`hello`"));
    assert!(get_statements_side_effect("const foo = ''; `hello${foo}`"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("`hello${foo}`"));
    assert!(get_statements_side_effect("const foo = {}; `hello${foo.bar}`"));
    assert!(get_statements_side_effect("tag`hello`"));
  }

  #[test]
  fn test_logical_expression() {
    assert!(!get_statements_side_effect("true && false"));
    assert!(!get_statements_side_effect("null ?? true"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("true && bar"));
    assert!(get_statements_side_effect("foo ?? true"));
  }

  #[test]
  fn test_parenthesized_expression() {
    assert!(!get_statements_side_effect("(true)"));
    assert!(!get_statements_side_effect("(null)"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("(bar)"));
    assert!(get_statements_side_effect("(foo)"));
  }

  #[test]
  fn test_sequence_expression() {
    assert!(!get_statements_side_effect("true, false"));
    assert!(!get_statements_side_effect("null, true"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("true, bar"));
    assert!(get_statements_side_effect("foo, true"));
  }

  #[test]
  fn test_conditional_expression() {
    assert!(!get_statements_side_effect("true ? false : true"));
    assert!(!get_statements_side_effect("null ? true : false"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("true ? bar : true"));
    assert!(get_statements_side_effect("foo ? true : false"));
    assert!(get_statements_side_effect("true ? bar : true"));
  }

  #[test]
  fn test_block_statement() {
    assert!(!get_statements_side_effect("{ }"));
    assert!(!get_statements_side_effect("{ const a = 1; }"));
    assert!(!get_statements_side_effect("{ const a = 1; const b = 2; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("{ const a = 1; bar; }"));
  }

  #[test]
  fn test_do_while_statement() {
    assert!(!get_statements_side_effect("do { } while (true)"));
    assert!(!get_statements_side_effect("do { const a = 1; } while (true)"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("do { const a = 1; } while (bar)"));
    assert!(get_statements_side_effect("do { const a = 1; bar; } while (true)"));
    assert!(get_statements_side_effect("do { bar; } while (true)"));
  }

  #[test]
  fn test_while_statement() {
    assert!(!get_statements_side_effect("while (true) { }"));
    assert!(!get_statements_side_effect("while (true) { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("while (bar) { const a = 1; }"));
    assert!(get_statements_side_effect("while (true) { const a = 1; bar; }"));
    assert!(get_statements_side_effect("while (true) { bar; }"));
  }

  #[test]
  fn test_if_statement() {
    assert!(!get_statements_side_effect("if (true) { }"));
    assert!(!get_statements_side_effect("if (true) { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("if (bar) { const a = 1; }"));
    assert!(get_statements_side_effect("if (true) { const a = 1; bar; }"));
    assert!(get_statements_side_effect("if (true) { bar; }"));
  }

  #[test]
  fn test_empty_statement() {
    assert!(!get_statements_side_effect(";"));
    assert!(!get_statements_side_effect(";;"));
  }

  #[test]
  fn test_continue_statement() {
    assert!(!get_statements_side_effect("continue;"));
  }

  #[test]
  fn test_break_statement() {
    assert!(!get_statements_side_effect("break;"));
  }

  #[test]
  fn test_return_statement() {
    assert!(!get_statements_side_effect("return;"));
    assert!(!get_statements_side_effect("return 1;"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("return bar;"));
  }

  #[test]
  fn test_labeled_statement() {
    assert!(!get_statements_side_effect("label: { }"));
    assert!(!get_statements_side_effect("label: { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("label: { const a = 1; bar; }"));
    assert!(get_statements_side_effect("label: { bar; }"));
  }

  #[test]
  fn test_try_statement() {
    assert!(!get_statements_side_effect("try { } catch (e) { }"));
    assert!(!get_statements_side_effect("try { const a = 1; } catch (e) { }"));
    assert!(!get_statements_side_effect("try { } catch (e) { const a = 1; }"));
    assert!(!get_statements_side_effect("try { const a = 1; } catch (e) { const a = 1; }"));
    assert!(!get_statements_side_effect("try { const a = 1; } finally { }"));
    assert!(!get_statements_side_effect("try { } catch (e) { const a = 1; } finally { }"));
    assert!(!get_statements_side_effect("try { } catch (e) { } finally { const a = 1; }"));
    assert!(!get_statements_side_effect(
      "try { const a = 1; } catch (e) { const a = 1; } finally { const a = 1; }"
    ));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("try { const a = 1; bar; } catch (e) { }"));
    assert!(get_statements_side_effect("try { } catch (e) { const a = 1; bar; }"));
    assert!(get_statements_side_effect("try { } catch (e) { bar; }"));
    assert!(get_statements_side_effect("try { const a = 1; } catch (e) { bar; }"));
    assert!(get_statements_side_effect("try { bar; } finally { }"));
    assert!(get_statements_side_effect("try { } catch (e) { bar; } finally { }"));
    assert!(get_statements_side_effect("try { } catch (e) { } finally { bar; }"));
    assert!(get_statements_side_effect("try { bar; } catch (e) { bar; } finally { bar; }"));
  }

  #[test]
  fn test_switch_statement() {
    assert!(!get_statements_side_effect("switch (true) { }"));
    assert!(!get_statements_side_effect("switch (true) { case 1: break; }"));
    assert!(!get_statements_side_effect("switch (true) { case 1: break; default: break; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("switch (bar) { case 1: break; }"));
    assert!(get_statements_side_effect("switch (true) { case 1: bar; }"));
    assert!(get_statements_side_effect("switch (true) { case bar: break; }"));
    assert!(get_statements_side_effect("switch (true) { case 1: bar; default: bar; }"));
  }

  #[test]
  fn test_binary_expression() {
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("1 + foo"));
    assert!(get_statements_side_effect("2 + bar"));
    assert!(!get_statements_side_effect("1 + 1"));
    assert!(get_statements_side_effect("const a = 1; const b = 2; a + b"));
  }

  #[test]
  fn test_private_in_expression() {
    assert!(get_statements_side_effect("#privateField in this"));
    assert!(get_statements_side_effect("const obj = {}; #privateField in obj"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("#privateField in bar"));
    assert!(get_statements_side_effect("#privateField in foo"));
  }

  #[test]
  fn test_this_expression() {
    assert!(!get_statements_side_effect("this"));
    assert!(get_statements_side_effect("this.a"));
    assert!(get_statements_side_effect("this.a + this.b"));
    assert!(get_statements_side_effect("this.a = 10"));
  }

  #[test]
  fn test_meta_property_expression() {
    assert!(!get_statements_side_effect("import.meta"));
    assert!(!get_statements_side_effect("const meta = import.meta"));
    assert!(!get_statements_side_effect("import.meta.url"));
    assert!(get_statements_side_effect("const { url } = import.meta"));
    assert!(get_statements_side_effect("import.meta.url = 'test'"));
  }

  #[test]
  fn test_assignment_expression() {
    assert!(get_statements_side_effect("let a; [] = a; ({} = a)"));
    assert!(get_statements_side_effect("let a; a = 1"));
    assert!(get_statements_side_effect("let a, b; a = b; a = b = 1"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("b = 1"));
    assert!(get_statements_side_effect("[] = b"));
    assert!(get_statements_side_effect("let a; a = b"));
    assert!(get_statements_side_effect("let a; a.b = 1"));
    assert!(get_statements_side_effect("let a; a['b'] = 1"));
    assert!(get_statements_side_effect("let a; a = a.b"));
    assert!(get_statements_side_effect("let a, b; ({ a } = b)"));
    assert!(get_statements_side_effect("let a, b; ({ ...a } = b)"));
    assert!(get_statements_side_effect("let a, b; [ a ] = b"));
    assert!(get_statements_side_effect("let a, b; [ ...a ] = b"));
  }

  #[test]
  fn test_chain_expression() {
    assert!(get_statements_side_effect("Object.create"));
    assert!(get_statements_side_effect("Object?.create"));
    assert!(!get_statements_side_effect("let a; /*#__PURE__*/ a?.()"));
    assert!(get_statements_side_effect("let a; a?.b"));
    assert!(get_statements_side_effect("let a; a?.()"));
    assert!(get_statements_side_effect("let a; a?.[a]"));
  }

  #[test]
  fn test_other_statements() {
    assert!(get_statements_side_effect("debugger;"));
    assert!(get_statements_side_effect("for (const k in {}) { }"));
    assert!(get_statements_side_effect("let a; for (const v of []) { a++ }"));
    assert!(get_statements_side_effect("for (;;) { }"));
    assert!(get_statements_side_effect("throw 1;"));
    assert!(get_statements_side_effect("with(a) { }"));
    assert!(get_statements_side_effect("await 1"));
    assert!(get_statements_side_effect("import('foo')"));
    assert!(get_statements_side_effect("let a; a``"));
    assert!(get_statements_side_effect("let a; a++"));
  }

  #[test]
  fn test_new_expr() {
    assert!(!get_statements_side_effect("new Map()"));
    assert!(!get_statements_side_effect("new Set()"));
    assert!(!get_statements_side_effect("new Map([[1, 2], [3, 4]]);"));
    assert!(get_statements_side_effect("new Regex()"));
    assert!(!get_statements_side_effect(
      "new Date(); new Date(''); new Date(null); new Date(false); new Date(undefined)"
    ));

    // TypedArray constructors should be side-effect free with no args, null, or undefined
    assert!(get_statements_side_effect("new Uint8Array()"));
    assert!(get_statements_side_effect("new Uint8Array(null)"));
    assert!(get_statements_side_effect("new Uint8Array(undefined)"));
    assert!(get_statements_side_effect("new Int8Array()"));
    assert!(get_statements_side_effect("new Uint16Array()"));
    assert!(get_statements_side_effect("new Uint32Array()"));
    assert!(get_statements_side_effect("new Float64Array()"));
    assert!(get_statements_side_effect("new BigUint64Array()"));

    // TypedArray constructors with numeric args should have side effects (memory allocation)
    assert!(get_statements_side_effect("new Uint8Array(10)"));
    assert!(get_statements_side_effect("new Int16Array(5)"));
    assert!(get_statements_side_effect("new Int32Array(100)"));
    assert!(get_statements_side_effect("new Float32Array(20)"));
    assert!(get_statements_side_effect("new BigInt64Array(8)"));
    assert!(get_statements_side_effect("new Uint8ClampedArray(256)"));

    // Symbol is not a constructor - using 'new' throws TypeError
    // All of these should have side effects (they throw errors)
    assert!(get_statements_side_effect("new Symbol()"));
    assert!(get_statements_side_effect("new Symbol('string')"));
    assert!(get_statements_side_effect("new Symbol(null)"));
    assert!(get_statements_side_effect("new Symbol(undefined)"));
    assert!(get_statements_side_effect("new Symbol({ toString() { throw new Error() } })"));
    assert!(get_statements_side_effect("let unknownVariable; new Symbol(unknownVariable)"));

    // Symbol() as a function call (without 'new') is side-effect-free with primitives
    assert!(!get_statements_side_effect("Symbol()"));
    assert!(!get_statements_side_effect("Symbol('string')"));
    assert!(!get_statements_side_effect("Symbol(null)"));
    assert!(!get_statements_side_effect("Symbol(undefined)"));
    assert!(!get_statements_side_effect("Symbol(123)"));
    assert!(!get_statements_side_effect("Symbol(true)"));

    // Symbol() with object argument has side effects (could call toString)
    assert!(get_statements_side_effect("Symbol({ toString() { throw new Error() } })"));

    // Symbol() with unknown variable has side effects (could be an object)
    assert!(get_statements_side_effect("let unknownVariable; Symbol(unknownVariable)"));

    // Test fallback logic for global constructors with primitive arguments
    // String, Number, Boolean, Object constructors are side-effect-free with primitives
    assert!(!get_statements_side_effect("new String()"));

    assert!(!get_statements_side_effect("new Number()"));

    assert!(!get_statements_side_effect("new Boolean()"));

    assert!(!get_statements_side_effect("new Object()"));

    assert!(get_statements_side_effect("new BigInt(123)"));
  }

  #[test]
  fn test_primitive_global_function_calls() {
    // String() - side-effect-free with primitive arguments only
    // Object conversion can call valueOf/toString with side effects
    assert!(!get_statements_side_effect("String()"));
    assert!(!get_statements_side_effect("String('hello')"));
    assert!(!get_statements_side_effect("String(123)"));
    assert!(!get_statements_side_effect("String(null)"));
    assert!(!get_statements_side_effect("String(undefined)"));
    assert!(!get_statements_side_effect("String(true)"));

    // String() with plain objects is side-effect-free under builtin assumptions.
    assert!(!get_statements_side_effect("String({})"));
    assert!(!get_statements_side_effect("String([1, 2, 3])"));
    assert!(get_statements_side_effect("let obj; String(obj)"));

    // Number() - side-effect-free with primitive arguments only
    assert!(!get_statements_side_effect("Number()"));
    assert!(!get_statements_side_effect("Number('123')"));
    assert!(!get_statements_side_effect("Number(456)"));
    assert!(!get_statements_side_effect("Number(null)"));
    assert!(!get_statements_side_effect("Number(undefined)"));
    assert!(!get_statements_side_effect("Number(true)"));

    // Number() with plain objects is side-effect-free under builtin assumptions.
    assert!(!get_statements_side_effect("Number({})"));
    assert!(get_statements_side_effect("let val; Number(val)"));

    // Boolean() - side-effect-free with primitive arguments only
    assert!(!get_statements_side_effect("Boolean()"));
    assert!(!get_statements_side_effect("Boolean(true)"));
    assert!(!get_statements_side_effect("Boolean('text')"));
    assert!(!get_statements_side_effect("Boolean(0)"));
    assert!(!get_statements_side_effect("Boolean(null)"));
    assert!(!get_statements_side_effect("Boolean(undefined)"));

    // Boolean() performs ToBoolean, which does not invoke user code.
    assert!(!get_statements_side_effect("Boolean({})"));
    assert!(!get_statements_side_effect("let val; Boolean(val)"));

    // BigInt() - side-effect-free only with proven-safe arguments
    // BigInt() with no arguments throws TypeError
    assert!(get_statements_side_effect("BigInt()"));
    // Integer literals are safe
    assert!(!get_statements_side_effect("BigInt(123)"));
    assert!(!get_statements_side_effect("BigInt(0)"));
    assert!(!get_statements_side_effect("BigInt(-1)"));
    assert!(!get_statements_side_effect("BigInt(+1)"));
    // Boolean literals are safe
    assert!(!get_statements_side_effect("BigInt(true)"));
    assert!(!get_statements_side_effect("BigInt(false)"));
    // BigInt literals are safe
    assert!(!get_statements_side_effect("BigInt(123n)"));

    // BigInt() with valid string literals is side-effect-free.
    assert!(!get_statements_side_effect("BigInt('456')"));
    assert!(get_statements_side_effect("BigInt('abc')"));

    // BigInt() with non-integer numbers throws RangeError
    assert!(get_statements_side_effect("BigInt(1.5)"));
    assert!(get_statements_side_effect("BigInt(NaN)"));
    assert!(get_statements_side_effect("BigInt(Infinity)"));
    assert!(get_statements_side_effect("BigInt(-Infinity)"));

    // BigInt() with undefined/null throws TypeError
    assert!(get_statements_side_effect("BigInt(undefined)"));
    assert!(get_statements_side_effect("BigInt(null)"));

    // BigInt() with unknown or object arguments has side effects
    assert!(get_statements_side_effect("let val; BigInt(val)"));
    assert!(get_statements_side_effect("BigInt({})"));

    // BigInt() with spread elements has side effects
    assert!(get_statements_side_effect("let args; BigInt(...args)"));

    // Spread elements should have side effects
    assert!(get_statements_side_effect("let args; String(...args)"));
    assert!(get_statements_side_effect("let args; Number(...args)"));
    assert!(get_statements_side_effect("let args; Boolean(...args)"));
  }

  #[test]
  fn test_regexp_constructor() {
    // RegExp() and new RegExp() with valid patterns/flags are side-effect-free
    // Valid patterns
    assert!(!get_statements_side_effect("RegExp()"));
    assert!(!get_statements_side_effect("new RegExp()"));
    assert!(!get_statements_side_effect("RegExp('abc')"));
    assert!(!get_statements_side_effect("new RegExp('abc')"));
    assert!(!get_statements_side_effect("RegExp('abc', 'g')"));
    assert!(!get_statements_side_effect("new RegExp('abc', 'g')"));
    assert!(!get_statements_side_effect("RegExp('abc', 'gi')"));
    assert!(!get_statements_side_effect("new RegExp('abc', 'gimsuy')"));
    // RegExp with a RegExp literal argument is valid
    assert!(!get_statements_side_effect("RegExp(/foo/)"));
    assert!(!get_statements_side_effect("new RegExp(/foo/)"));

    // Invalid patterns throw SyntaxError - these have side effects
    assert!(get_statements_side_effect("RegExp('[')"));
    assert!(get_statements_side_effect("new RegExp('[')"));
    assert!(get_statements_side_effect("RegExp('\\\\')"));
    assert!(get_statements_side_effect("new RegExp('\\\\')"));

    // Invalid flags throw SyntaxError - these have side effects
    assert!(get_statements_side_effect("RegExp('a', 'xyz')"));
    assert!(get_statements_side_effect("new RegExp('a', 'xyz')"));
    assert!(get_statements_side_effect("RegExp('a', 'gg')"));
    assert!(get_statements_side_effect("new RegExp('a', 'gg')"));

    // Non-literal arguments have side effects (can't statically validate)
    assert!(get_statements_side_effect("let p; RegExp(p)"));
    assert!(get_statements_side_effect("let p; new RegExp(p)"));
    assert!(get_statements_side_effect("let f; RegExp('a', f)"));
    assert!(get_statements_side_effect("let f; new RegExp('a', f)"));

    // RegExp literals are side-effect-free (they're validated at parse time)
    assert!(!get_statements_side_effect("/abc/"));
    assert!(!get_statements_side_effect("/abc/g"));
  }

  #[test]
  fn test_side_effects_of_global_variable_access() {
    assert!(!get_statements_side_effect("let a = undefined"));
    assert!(!get_statements_side_effect("let a = void 0"));
    assert!(!get_statements_side_effect("using undef_remove = void 0;"));
    assert!(get_statements_side_effect("using undef_keep = void test();"));
    assert!(!get_statements_side_effect("let a = NaN"));
    assert!(get_statements_side_effect("let a = String"));
    assert!(get_statements_side_effect("let a = Object.assign"));
    assert!(get_statements_side_effect("let a = Object.prototype.propertyIsEnumerable"));
    assert!(get_statements_side_effect("let a = Symbol.asyncDispose"));
    assert!(get_statements_side_effect("let a = Math.E"));
    assert!(get_statements_side_effect("let a = Reflect.apply"));
    assert!(get_statements_side_effect("let a = JSON.stringify"));
    assert!(get_statements_side_effect("let a = Proxy"));

    assert_eq!(
      get_statements_side_effect_details("let a = Proxy; let a = JSON.stringify"),
      vec![
        SideEffectDetail::Unknown | SideEffectDetail::GlobalVarAccess,
        SideEffectDetail::Unknown | SideEffectDetail::GlobalVarAccess
      ]
    );
    // should have side effects other global member expr access
    assert!(get_statements_side_effect("let a = Object.test"));
    assert!(get_statements_side_effect("let a = Object.prototype.two"));
    assert!(get_statements_side_effect("let a = Reflect.something"));

    assert_eq!(
      get_statements_side_effect_details("let a = Reflect.something"),
      vec![SideEffectDetail::Unknown | SideEffectDetail::GlobalVarAccess]
    );

    // sideEffectful Global variable access with pure annotation
    assert_eq!(
      get_statements_side_effect_details("let a = /*@__PURE__ */ Reflect.something()"),
      vec![SideEffectDetail::GlobalVarAccess | SideEffectDetail::PureAnnotation]
    );
  }

  #[test]
  fn test_object_expression() {
    assert!(!get_statements_side_effect("const of = { [1]: 'hi'}"));
    assert!(!get_statements_side_effect("const of = { [-1]: 'hi'}"));
    assert!(!get_statements_side_effect("const of = { [+1]: 'hi'}"));
    assert!(!get_statements_side_effect("let remove = { [void 0]: 'x' };"));
    assert!(get_statements_side_effect("let keep = { [void test()]: 'x' };"));
    assert!(!get_statements_side_effect("const of = { [{}]: 'hi'}"));
  }

  #[test]
  fn test_cjs_pattern() {
    assert_eq!(
      get_statements_side_effect_details(
        "Object.defineProperty(exports, \"__esModule\", { value: true })"
      ),
      vec![SideEffectDetail::Unknown | SideEffectDetail::GlobalVarAccess]
    );

    assert_eq!(
      get_statements_side_effect_details(
        r"
      exports.a = function test() {};
      exports['b'] = function () {
        console.log('b')
      };
      "
      ),
      vec![SideEffectDetail::PureCjs, SideEffectDetail::PureCjs]
    );

    assert_eq!(
      get_statements_side_effect_details("exports.a = global()"),
      vec![
        SideEffectDetail::Unknown | SideEffectDetail::PureCjs | SideEffectDetail::GlobalVarAccess
      ]
    );

    assert_eq!(
      get_statements_side_effect_details("exports[test()] = true"),
      vec![SideEffectDetail::Unknown | SideEffectDetail::GlobalVarAccess]
    );

    assert_eq!(
      get_statements_side_effect_details(
        r"
      let a = {};
      Object.defineProperty(a, '__esModule', { value: true });
      "
      ),
      vec![
        SideEffectDetail::empty(),
        SideEffectDetail::Unknown | SideEffectDetail::GlobalVarAccess,
      ]
    );
  }

  #[test]
  fn test_class_expr() {
    assert!(!get_statements_side_effect(
      r"
let remove14 = class {
	static [undefined] = 'x';
}

let remove15 = class {
	static [void 0] = 'x';
}

let remove15 = class {
	[void 0] = 'x';
}
    "
    ));
  }

  #[test]
  fn test_class_decorators() {
    assert!(get_statements_side_effect("function fn() {} @fn class Class {}"));
    assert!(get_statements_side_effect("function fn() {} var MyClass = @fn class {}"));
    assert!(get_statements_side_effect("function fn() {} class MyClass { @fn accessor x }"));
    assert!(get_statements_side_effect("function fn() {} class MyClass { @fn static accessor x }"));
    assert!(get_statements_side_effect("function fn() {} class MyClass { @fn method() {} }"));
    assert!(get_statements_side_effect("function fn() {} class MyClass { @fn field }"));
  }

  #[test]
  fn test_extract_first_part_of_member_expr_like() {
    assert!(extract_first_part_of_member_expr_like_helper("a.b") == "a");
    assert!(extract_first_part_of_member_expr_like_helper("styled?.div()") == "styled");
    assert!(extract_first_part_of_member_expr_like_helper("styled()") == "styled");
    assert!(extract_first_part_of_member_expr_like_helper("styled().div") == "styled");
    assert!(extract_first_part_of_member_expr_like_helper("styled()()") == "styled");
  }

  fn extract_first_part_of_member_expr_like_helper(code: &str) -> String {
    let allocator = oxc::allocator::Allocator::default();
    let parser = Parser::new(&allocator, code, SourceType::ts());
    let expr = parser.parse_expression().unwrap();
    SideEffectDetector::extract_first_part_of_member_expr_like(&expr).unwrap().to_string()
  }
}
