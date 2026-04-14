use oxc::ast::ast::{
  self, Argument, AssignmentTarget, BindingPattern, CallExpression, ChainElement, Expression,
  IdentifierReference, UnaryOperator, VariableDeclarationKind,
};
use oxc::ast::match_member_expression;
use oxc::semantic::SymbolId;
use oxc_allocator::{Address, UnstableAddress};
use oxc_ecmascript::GlobalContext;
use oxc_ecmascript::side_effects::{
  MayHaveSideEffects, MayHaveSideEffectsContext, PropertyReadSideEffects,
};
use rolldown_common::{AstScopes, FlatOptions, SharedNormalizedBundlerOptions, SideEffectDetail};
use rolldown_ecmascript_utils::ExpressionExt;
use rustc_hash::FxHashSet;

/// Detect if a statement "may" have side effect.
pub struct SideEffectDetector<'a> {
  scope: &'a AstScopes,
  options: &'a SharedNormalizedBundlerOptions,
  flat_options: FlatOptions,
  /// Cross-module optimization: addresses of call expressions to known-pure functions.
  side_effect_free_call_expr_addr: Option<&'a FxHashSet<Address>>,
  /// Symbol IDs of namespace imports (`import * as ns from '...'`).
  /// Property reads on ES module namespace objects are guaranteed side-effect-free
  /// because namespace objects are frozen/sealed by spec with no getters.
  namespace_object_symbol_ids: Option<&'a FxHashSet<SymbolId>>,
}

impl<'a> SideEffectDetector<'a> {
  pub fn new(
    scope: &'a AstScopes,
    flat_options: FlatOptions,
    options: &'a SharedNormalizedBundlerOptions,
    side_effect_free_call_expr_addr: Option<&'a FxHashSet<Address>>,
    namespace_object_symbol_ids: Option<&'a FxHashSet<SymbolId>>,
  ) -> Self {
    Self {
      scope,
      options,
      flat_options,
      side_effect_free_call_expr_addr,
      namespace_object_symbol_ids,
    }
  }

  /// Check if a call expression has been marked pure by cross-module optimization.
  fn is_call_expr_marked_pure(&self, expr: &CallExpression) -> bool {
    self.side_effect_free_call_expr_addr.is_some_and(|set| set.contains(&expr.unstable_address()))
  }

  #[inline]
  fn is_unresolved_reference(&self, ident_ref: &IdentifierReference) -> bool {
    ident_ref.reference_id.get().is_some_and(|ref_id| self.scope.is_unresolved(ref_id))
  }

  /// Walk a member expression chain to find whether the root is an unresolved global.
  fn is_member_expr_root_global(&self, expr: &ast::MemberExpression) -> bool {
    let mut cur = expr.object();
    loop {
      match cur {
        Expression::StaticMemberExpression(e) => cur = &e.object,
        Expression::ComputedMemberExpression(e) => cur = &e.object,
        Expression::Identifier(ident) => return self.is_unresolved_reference(ident),
        _ => return false,
      }
    }
  }

  /// Check if the member expression's direct object is an ES module namespace import.
  /// ES module namespace objects are frozen/sealed by spec — property reads on them
  /// can never have side effects (no getters possible).
  fn is_namespace_member_access(&self, member_expr: &ast::MemberExpression) -> Option<bool> {
    let namespace_ids = self.namespace_object_symbol_ids?;
    let ident = member_expr.object().as_identifier()?;
    let ref_id = ident.reference_id.get()?;
    let symbol_id = self.scope.symbol_id_for(ref_id)?;
    Some(namespace_ids.contains(&symbol_id))
  }

  fn detect_side_effect_of_member_expr(
    &self,
    member_expr: &ast::MemberExpression,
  ) -> SideEffectDetail {
    if self.is_expr_manual_pure_functions(member_expr.object()) {
      return false.into();
    }
    // ES module namespace objects are frozen/sealed by spec — property reads
    // on them are guaranteed side-effect-free.
    if self.is_namespace_member_access(member_expr) == Some(true) {
      return false.into();
    }
    // Only `import.meta.url` is a spec-defined side-effect-free property read.
    // Other accesses like `import.meta.hot.accept()` may have side effects.
    if let ast::MemberExpression::StaticMemberExpression(static_expr) = member_expr {
      if matches!(static_expr.object, Expression::MetaProperty(_))
        && static_expr.property.name == "url"
      {
        return false.into();
      }
    }
    let is_global = self.is_member_expr_root_global(member_expr);
    let has_side_effect = member_expr.may_have_side_effects(self);
    let mut detail = SideEffectDetail::from(has_side_effect);
    detail.set(SideEffectDetail::GlobalVarAccess, is_global);
    detail
  }

  /// Collect `GlobalVarAccess` metadata from a member-like write target.
  /// Called after Oxc has determined the expression is side-effect-free.
  /// Writing to a property of an unresolved global mutates shared state.
  fn collect_write_target_metadata(
    &self,
    target: &ast::SimpleAssignmentTarget,
  ) -> SideEffectDetail {
    let object_detail = match target {
      ast::SimpleAssignmentTarget::StaticMemberExpression(e) => {
        self.detect_side_effect_of_expr(&e.object)
      }
      ast::SimpleAssignmentTarget::ComputedMemberExpression(e) => {
        self.detect_side_effect_of_expr(&e.object) | self.detect_side_effect_of_expr(&e.expression)
      }
      ast::SimpleAssignmentTarget::PrivateFieldExpression(e) => {
        self.detect_side_effect_of_expr(&e.object)
      }
      _ => return false.into(),
    };
    if object_detail.contains(SideEffectDetail::GlobalVarAccess) {
      object_detail | true.into()
    } else {
      object_detail
    }
  }

  fn detect_side_effect_of_call_expr(&self, expr: &CallExpression) -> SideEffectDetail {
    let is_pure_annotated =
      !self.flat_options.ignore_annotations() && (expr.pure || self.is_call_expr_marked_pure(expr));

    // For pure-annotated calls, the call itself is side-effect-free.
    // We must check args via Rolldown's detector (not Oxc's) because Rolldown
    // has bundler-specific overrides (e.g. import.meta.url is side-effect-free).
    // Oxc's pure-call handling would still check args via its own may_have_side_effects,
    // which doesn't know about these overrides.
    let has_side_effect = if is_pure_annotated { false } else { expr.may_have_side_effects(self) };

    let is_global_call = !has_side_effect
      && matches!(&expr.callee, Expression::Identifier(id) if self.is_unresolved_reference(id));

    let mut detail = SideEffectDetail::from(has_side_effect);
    detail.set(SideEffectDetail::PureAnnotation, is_pure_annotated);
    detail.set(SideEffectDetail::GlobalVarAccess, is_global_call);

    if !has_side_effect {
      // Strip the Unknown flag from callee since the call itself is known-pure/safe.
      detail |= self.detect_side_effect_of_expr(&expr.callee) - SideEffectDetail::Unknown;

      if is_pure_annotated {
        // Pure-annotated calls bypass Oxc's arg checking, so we must check args
        // through Rolldown's detector which has bundler-specific overrides
        // (e.g. import.meta.url is side-effect-free).
        for arg in &expr.arguments {
          detail |= match arg {
            Argument::SpreadElement(_) => true.into(),
            _ => self.detect_side_effect_of_expr(arg.to_expression()),
          };
          if detail.has_side_effect() {
            break;
          }
        }
      } else {
        // Oxc already verified args are side-effect-free; only collect metadata flags.
        for arg in &expr.arguments {
          if let Argument::SpreadElement(_) = arg {
            break;
          }
          detail |=
            self.detect_side_effect_of_expr(arg.to_expression()) - SideEffectDetail::Unknown;
        }
      }
    }
    detail
  }

  fn is_expr_manual_pure_functions(&self, expr: &Expression) -> bool {
    if self.flat_options.is_manual_pure_functions_empty() {
      return false;
    }
    let manual_pure_functions = self.options.treeshake.manual_pure_functions().unwrap();
    extract_first_part_of_member_expr_like(expr)
      .is_some_and(|first| manual_pure_functions.contains(first))
  }

  fn detect_side_effect_of_expr(&self, expr: &Expression) -> SideEffectDetail {
    match expr {
      // --- Bundler-specific overrides (metadata or custom logic) ---
      oxc::ast::match_member_expression!(Expression) => {
        self.detect_side_effect_of_member_expr(expr.to_member_expression())
      }
      Expression::Identifier(ident) => self.detect_side_effect_of_identifier(ident),
      Expression::AssignmentExpression(assign_expr) => {
        // Bundler-specific: CJS `exports.foo = ...` must be checked before Oxc,
        // because Oxc would see a write to an unresolved global and return true.
        if let Some(pure_cjs) = check_pure_cjs_export(self.scope, &assign_expr.left) {
          return pure_cjs | self.detect_side_effect_of_expr(&assign_expr.right);
        }
        if assign_expr.may_have_side_effects(self) {
          return true.into();
        }
        // Oxc says side-effect-free; collect GlobalVarAccess metadata.
        self.collect_write_target_metadata(assign_expr.left.to_simple_assignment_target())
      }

      Expression::ChainExpression(chain_expr) => match &chain_expr.expression {
        ChainElement::CallExpression(call_expr) => self.detect_side_effect_of_call_expr(call_expr),
        ChainElement::TSNonNullExpression(ts_expr) => {
          self.detect_side_effect_of_expr(&ts_expr.expression)
        }
        match_member_expression!(ChainElement) => {
          self.detect_side_effect_of_member_expr(chain_expr.expression.to_member_expression())
        }
      },
      Expression::UpdateExpression(update_expr) => {
        if update_expr.may_have_side_effects(self) {
          return true.into();
        }
        // Oxc says side-effect-free; collect GlobalVarAccess metadata.
        self.collect_write_target_metadata(&update_expr.argument)
      }
      Expression::NewExpression(expr) => {
        let has_side_effect = expr.may_have_side_effects(self);

        // METADATA: GlobalVarAccess — constructor is a known global
        let is_global_constructor = !has_side_effect
          && matches!(&expr.callee, Expression::Identifier(id) if self.is_unresolved_reference(id));
        // METADATA: PureAnnotation — marked with /*@__PURE__*/
        let is_pure_annotated = !self.flat_options.ignore_annotations() && expr.pure;

        let mut detail = SideEffectDetail::from(has_side_effect);
        detail.set(SideEffectDetail::GlobalVarAccess, is_global_constructor);
        detail.set(SideEffectDetail::PureAnnotation, is_pure_annotated);

        if !has_side_effect {
          // Oxc already verified args are side-effect-free; only collect metadata flags.
          for arg in &expr.arguments {
            if let Argument::SpreadElement(_) = arg {
              break;
            }
            detail |=
              self.detect_side_effect_of_expr(arg.to_expression()) - SideEffectDetail::Unknown;
          }
        }
        detail
      }
      Expression::CallExpression(expr) => self.detect_side_effect_of_call_expr(expr),
      // Everything else: delegate entirely to Oxc.
      // This covers literals, object/array/class expressions, unary/binary/logical/
      // conditional/sequence/template/tagged-template/parenthesized expressions,
      // TS/JSX syntax, await/import/yield, and any future expression types.
      _ => expr.may_have_side_effects(self).into(),
    }
  }

  fn detect_side_effect_of_var_decl(
    &self,
    var_decl: &ast::VariableDeclaration,
  ) -> SideEffectDetail {
    match var_decl.kind {
      VariableDeclarationKind::AwaitUsing => true.into(),
      VariableDeclarationKind::Using => {
        self.detect_side_effect_of_using_declarators(&var_decl.declarations)
      }
      _ => {
        let mut detail = SideEffectDetail::empty();
        for declarator in &var_decl.declarations {
          detail |= match &declarator.id {
            BindingPattern::ObjectPattern(_) if self.flat_options.property_read_side_effects() => {
              true.into()
            }
            BindingPattern::ArrayPattern(pat)
              if pat.elements.iter().any(|p| {
                p.as_ref().is_some_and(|pat| !matches!(pat, BindingPattern::BindingIdentifier(_)))
              }) =>
            {
              true.into()
            }
            _ => declarator
              .init
              .as_ref()
              .map(|init| self.detect_side_effect_of_expr(init))
              .unwrap_or(false.into()),
          };
        }
        detail
      }
    }
  }

  fn detect_side_effect_of_decl(&self, decl: &ast::Declaration) -> SideEffectDetail {
    match decl {
      ast::Declaration::VariableDeclaration(var_decl) => {
        self.detect_side_effect_of_var_decl(var_decl)
      }
      _ => decl.may_have_side_effects(self).into(),
    }
  }

  fn detect_side_effect_of_using_declarators(
    &self,
    declarators: &[ast::VariableDeclarator],
  ) -> SideEffectDetail {
    let mut detail = SideEffectDetail::empty();
    for decl in declarators {
      detail |= decl
        .init
        .as_ref()
        .map(|init| match init {
          Expression::NullLiteral(_) => false.into(),
          // Side effect detection of identifier is different with other position when as initialization of using declaration.
          // Global variable `undefined` is considered as side effect free.
          Expression::Identifier(id) => {
            (!(id.name == "undefined" && self.is_unresolved_reference(id))).into()
          }
          Expression::UnaryExpression(expr) if matches!(expr.operator, UnaryOperator::Void) => {
            self.detect_side_effect_of_expr(&expr.argument)
          }
          _ => true.into(),
        })
        .unwrap_or(SideEffectDetail::empty());
      if detail.has_side_effect() {
        break;
      }
    }
    detail
  }

  fn detect_side_effect_of_identifier(&self, ident_ref: &IdentifierReference) -> SideEffectDetail {
    let is_global = self.is_unresolved_reference(ident_ref);
    // Delegate side-effect bool to Oxc (checks known globals, unknown_global_side_effects)
    let has_side_effect = ident_ref.may_have_side_effects(self);
    let mut detail = SideEffectDetail::from(has_side_effect);
    // METADATA: GlobalVarAccess
    detail.set(SideEffectDetail::GlobalVarAccess, is_global);
    detail
  }

  /// Bundler-specific: module declarations like import/export are treated
  /// differently than in generic JS analysis.
  /// - import/export-all/re-export: side-effect-free (bundler handles them)
  /// - export default: recurse into declaration
  /// - export named with source: side-effect-free
  fn detect_side_effect_of_module_declaration(
    &self,
    decl: &ast::ModuleDeclaration,
  ) -> SideEffectDetail {
    match decl {
      ast::ModuleDeclaration::ExportAllDeclaration(_)
      | ast::ModuleDeclaration::ImportDeclaration(_) => {
        // We consider `import ...` has no side effect. However, `import ...` might be rewritten to other statements by the bundler.
        // In that case, we will mark the statement as having side effect in link stage.
        false.into()
      }
      ast::ModuleDeclaration::ExportDefaultDeclaration(default_decl) => {
        use oxc::ast::ast::ExportDefaultDeclarationKind;
        match &default_decl.declaration {
          decl @ oxc::ast::match_expression!(ExportDefaultDeclarationKind) => {
            self.detect_side_effect_of_expr(decl.to_expression())
          }
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(_) => false.into(),
          ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
            decl.may_have_side_effects(self).into()
          }
          ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => true.into(),
        }
      }
      ast::ModuleDeclaration::ExportNamedDeclaration(named_decl) => {
        if named_decl.source.is_some() {
          false.into()
        } else {
          named_decl
            .declaration
            .as_ref()
            .map(|decl| self.detect_side_effect_of_decl(decl))
            .unwrap_or(false.into())
        }
      }
      ast::ModuleDeclaration::TSExportAssignment(_)
      | ast::ModuleDeclaration::TSNamespaceExportDeclaration(_) => true.into(),
    }
  }

  pub fn detect_side_effect_of_stmt(&self, stmt: &ast::Statement) -> SideEffectDetail {
    use oxc::ast::ast::Statement;
    match stmt {
      // Bundler-specific: module declarations
      oxc::ast::match_module_declaration!(Statement) => {
        self.detect_side_effect_of_module_declaration(stmt.to_module_declaration())
      }
      // Language-level: everything else
      oxc::ast::match_declaration!(Statement) => {
        self.detect_side_effect_of_decl(stmt.to_declaration())
      }
      Statement::ExpressionStatement(expr) => self.detect_side_effect_of_expr(&expr.expression),
      Statement::BlockStatement(block) => self.detect_side_effect_of_block(block),
      Statement::DoWhileStatement(do_while) => {
        self.detect_side_effect_of_stmt(&do_while.body)
          | self.detect_side_effect_of_expr(&do_while.test)
      }
      Statement::WhileStatement(while_stmt) => {
        self.detect_side_effect_of_expr(&while_stmt.test)
          | self.detect_side_effect_of_stmt(&while_stmt.body)
      }
      Statement::IfStatement(if_stmt) => {
        self.detect_side_effect_of_expr(&if_stmt.test)
          | self.detect_side_effect_of_stmt(&if_stmt.consequent)
          | if_stmt
            .alternate
            .as_ref()
            .map(|s| self.detect_side_effect_of_stmt(s))
            .unwrap_or(false.into())
      }
      Statement::ReturnStatement(ret_stmt) => ret_stmt
        .argument
        .as_ref()
        .map(|expr| self.detect_side_effect_of_expr(expr))
        .unwrap_or(false.into()),
      Statement::LabeledStatement(labeled_stmt) => {
        self.detect_side_effect_of_stmt(&labeled_stmt.body)
      }
      Statement::TryStatement(try_stmt) => {
        let mut detail = self.detect_side_effect_of_block(&try_stmt.block);
        detail |= try_stmt
          .handler
          .as_ref()
          .map(|handler| self.detect_side_effect_of_block(&handler.body))
          .unwrap_or(SideEffectDetail::empty());
        detail |= try_stmt
          .finalizer
          .as_ref()
          .map(|finalizer| self.detect_side_effect_of_block(finalizer))
          .unwrap_or(SideEffectDetail::empty());
        detail
      }
      Statement::SwitchStatement(switch_stmt) => {
        let mut detail = self.detect_side_effect_of_expr(&switch_stmt.discriminant);
        if detail.has_side_effect() {
          return detail;
        }
        'outer: for case in &switch_stmt.cases {
          detail |= case
            .test
            .as_ref()
            .map(|expr| self.detect_side_effect_of_expr(expr))
            .unwrap_or(SideEffectDetail::empty());
          for stmt in &case.consequent {
            detail |= self.detect_side_effect_of_stmt(stmt);
            if detail.has_side_effect() {
              break 'outer;
            }
          }
        }
        detail
      }
      // Everything else: delegate to Oxc.
      // This covers Empty, Continue, Break, Debugger, For/ForIn/ForOf, Throw, With.
      _ => stmt.may_have_side_effects(self).into(),
    }
  }

  fn detect_side_effect_of_block(&self, block: &ast::BlockStatement) -> SideEffectDetail {
    let mut detail = SideEffectDetail::empty();
    for stmt in &block.body {
      detail |= self.detect_side_effect_of_stmt(stmt);
      if detail.has_side_effect() {
        break;
      }
    }
    detail
  }
}

/// Bundler-specific: detect `exports.staticProp = ...` CJS export pattern.
/// Returns `Some(PureCjs)` if the target matches, `None` otherwise.
fn check_pure_cjs_export(scope: &AstScopes, target: &AssignmentTarget) -> Option<SideEffectDetail> {
  match target {
    AssignmentTarget::ComputedMemberExpression(_) | AssignmentTarget::StaticMemberExpression(_) => {
      let member_expr = target.to_member_expression();
      if let Expression::Identifier(ident) = member_expr.object() {
        if ident.reference_id.get().is_some_and(|ref_id| scope.is_unresolved(ref_id))
          && ident.name == "exports"
          && member_expr.static_property_name().is_some()
        {
          return Some(SideEffectDetail::PureCjs);
        }
      }
      None
    }
    _ => None,
  }
}

/// Extract the first (leftmost) identifier name from a member expression chain.
/// Used by both `SideEffectDetector::is_expr_manual_pure_functions` and
/// `SideEffectDetector::manual_pure_functions`.
fn extract_first_part_of_member_expr_like<'a>(expr: &'a Expression) -> Option<&'a str> {
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

impl GlobalContext<'_> for SideEffectDetector<'_> {
  fn is_global_reference(&self, reference: &IdentifierReference<'_>) -> bool {
    self.is_unresolved_reference(reference)
  }
}

impl MayHaveSideEffectsContext<'_> for SideEffectDetector<'_> {
  fn annotations(&self) -> bool {
    !self.flat_options.ignore_annotations()
  }

  fn manual_pure_functions(&self, callee: &Expression) -> bool {
    self.is_expr_manual_pure_functions(callee)
  }

  fn property_read_side_effects(&self) -> PropertyReadSideEffects {
    if self.flat_options.property_read_side_effects() {
      PropertyReadSideEffects::All
    } else {
      PropertyReadSideEffects::None
    }
  }

  fn unknown_global_side_effects(&self) -> bool {
    self.options.treeshake.unknown_global_side_effects()
  }

  fn property_write_side_effects(&self) -> bool {
    self.flat_options.property_write_side_effects()
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
      SideEffectDetector::new(&ast_scopes, flags, &options, None, None)
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
        SideEffectDetector::new(&ast_scopes, flags, &options, None, None)
          .detect_side_effect_of_stmt(stmt)
      })
      .collect_vec()
  }

  #[test]
  fn test_side_effect() {
    assert!(!get_statements_side_effect("export { a }"));
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
    // Oxc correctly recognizes primitive literal operands as side-effect-free
    assert!(!get_statements_side_effect("1 + 1"));
    // Oxc doesn't do constant propagation through variables, so `a + b` is
    // conservatively treated as potentially side-effectful (ToPrimitive)
    assert!(get_statements_side_effect("const a = 1; const b = 2; a + b"));
  }

  #[test]
  fn test_private_in_expression() {
    // Oxc checks that the RHS is known to be an object; `this` and local
    // variables with unknown value type are conservatively treated as
    // potentially non-object, so `#x in this` / `#x in obj` may throw.
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
    // Other import.meta properties are not spec-defined as side-effect-free
    assert!(get_statements_side_effect("import.meta.hot"));
    // Deeper chains may throw (e.g. import.meta.nonExisting is undefined, .foo throws TypeError)
    assert!(get_statements_side_effect("import.meta.nonExisting.foo"));
    assert!(get_statements_side_effect("const { url } = import.meta"));
    assert!(get_statements_side_effect("import.meta.url = 'test'"));
  }

  #[test]
  fn test_assignment_expression() {
    // Destructuring assignments are side-effectful (GetIterator / RequireObjectCoercible).
    assert!(get_statements_side_effect("let a; [] = a"));
    assert!(get_statements_side_effect("({} = a)"));
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
    assert!(!get_statements_side_effect("Object.create"));
    assert!(!get_statements_side_effect("Object?.create"));
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
    assert!(!get_statements_side_effect("new Uint8Array()"));
    assert!(!get_statements_side_effect("new Uint8Array(null)"));
    assert!(!get_statements_side_effect("new Uint8Array(undefined)"));
    assert!(!get_statements_side_effect("new Int8Array()"));
    assert!(!get_statements_side_effect("new Uint16Array()"));
    assert!(!get_statements_side_effect("new Uint32Array()"));
    assert!(!get_statements_side_effect("new Float64Array()"));
    assert!(!get_statements_side_effect("new BigUint64Array()"));

    // TypedArray constructors with numeric args are side-effect free
    // (memory allocation is not an observable side effect for tree-shaking)
    assert!(!get_statements_side_effect("new Uint8Array(10)"));
    assert!(!get_statements_side_effect("new Int16Array(5)"));
    assert!(!get_statements_side_effect("new Int32Array(100)"));
    assert!(!get_statements_side_effect("new Float32Array(20)"));
    assert!(!get_statements_side_effect("new BigInt64Array(8)"));
    assert!(!get_statements_side_effect("new Uint8ClampedArray(256)"));

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

    // String() with any value: Oxc's "coercion methods are pure" assumption
    // treats toString()/valueOf() as side-effect-free. String(Symbol()) is also
    // safe per spec (returns "Symbol()" without throwing).
    assert!(!get_statements_side_effect("String({})"));
    assert!(!get_statements_side_effect("String([1, 2, 3])"));
    assert!(!get_statements_side_effect("let obj; String(obj)"));

    // Number() - side-effect-free with primitive arguments only
    assert!(!get_statements_side_effect("Number()"));
    assert!(!get_statements_side_effect("Number('123')"));
    assert!(!get_statements_side_effect("Number(456)"));
    assert!(!get_statements_side_effect("Number(null)"));
    assert!(!get_statements_side_effect("Number(undefined)"));
    assert!(!get_statements_side_effect("Number(true)"));

    // Number() with object literals: Oxc checks ToPrimitive/ToNumeric.
    // {} has known valueOf/toString, so ToNumeric({}) = NaN (no throw).
    assert!(!get_statements_side_effect("Number({})"));
    assert!(get_statements_side_effect("let val; Number(val)"));

    // Boolean() - always side-effect free (no type conversion needed)
    assert!(!get_statements_side_effect("Boolean()"));
    assert!(!get_statements_side_effect("Boolean(true)"));
    assert!(!get_statements_side_effect("Boolean('text')"));
    assert!(!get_statements_side_effect("Boolean(0)"));
    assert!(!get_statements_side_effect("Boolean(null)"));
    assert!(!get_statements_side_effect("Boolean(undefined)"));

    // Boolean() with any value is side-effect free (just checks truthiness)
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

    // BigInt() with strings: Oxc can statically validate integer strings.
    // BigInt("123") works, BigInt("abc") or BigInt("1.5") throws.
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
    assert!(!get_statements_side_effect("let a = String"));
    assert!(!get_statements_side_effect("let a = Object.assign"));
    assert!(!get_statements_side_effect("let a = Object.prototype.propertyIsEnumerable"));
    assert!(!get_statements_side_effect("let a = Symbol.asyncDispose"));
    assert!(!get_statements_side_effect("let a = Math.E"));
    assert!(!get_statements_side_effect("let a = Reflect.apply"));
    assert!(!get_statements_side_effect("let a = JSON.stringify"));
    assert!(!get_statements_side_effect("let a = Proxy"));

    assert_eq!(
      get_statements_side_effect_details("let a = Proxy; let a = JSON.stringify"),
      vec![SideEffectDetail::GlobalVarAccess, SideEffectDetail::GlobalVarAccess]
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
    // Oxc is more permissive about computed property keys (ignores ToPrimitive side effects).
    // `{}` has a known toString(), so Oxc considers this side-effect-free.
    assert!(!get_statements_side_effect("const of = { [{}]: 'hi'}"));
  }

  #[test]
  fn test_cjs_pattern() {
    assert_eq!(
      get_statements_side_effect_details(
        "Object.defineProperty(exports, \"__esModule\", { value: true })"
      ),
      vec![SideEffectDetail::Unknown]
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
      vec![SideEffectDetail::Unknown | SideEffectDetail::PureCjs]
    );

    assert_eq!(
      get_statements_side_effect_details("exports[test()] = true"),
      vec![SideEffectDetail::Unknown]
    );

    assert_eq!(
      get_statements_side_effect_details(
        r"
      let a = {};
      Object.defineProperty(a, '__esModule', { value: true });
      "
      ),
      vec![SideEffectDetail::empty(), SideEffectDetail::Unknown]
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
    super::extract_first_part_of_member_expr_like(&expr).unwrap().to_string()
  }
}
