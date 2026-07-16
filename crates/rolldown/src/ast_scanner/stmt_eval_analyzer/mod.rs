use std::ops::{BitOr, BitOrAssign};

use bitflags::bitflags;
use oxc::ast::ast::{
  self, Argument, AssignmentTarget, BindingPattern, CallExpression, ChainElement, Expression,
  IdentifierReference, UnaryOperator, VariableDeclarationKind,
};
use oxc::ast::match_member_expression;
use oxc::semantic::{NodeId, SymbolId};
use oxc_ecmascript::GlobalContext;
use oxc_ecmascript::side_effects::{
  MayHaveSideEffects, MayHaveSideEffectsContext, PropertyReadSideEffects,
};
use rolldown_common::{AstScopes, FlatOptions, SharedNormalizedBundlerOptions, StmtEvalFlags};
use rolldown_ecmascript_utils::ExpressionExt;
use rustc_hash::FxHashSet;

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
    /// Reasons that make an otherwise side-effect-free statement sensitive to execution order.
    struct StmtOrderSensitiveReasons: u8 {
        /// Reads from an unresolved global or a member chain rooted at one.
        const GlobalVarAccess = 1;
        /// A call/new expression was marked pure by an annotation or cross-module analysis.
        const PureAnnotation = 1 << 1;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
/// Evaluation facts for a statement.
///
/// `tree_shaking_flags` answers whether the statement must be retained. `is_order_sensitive`
/// combines unknown tree-shaking side effects with order-sensitive reasons to answer whether the
/// statement's relative execution timing matters.
pub struct StmtEvalFacts {
  tree_shaking_flags: StmtEvalFlags,
  order_sensitive_reasons: StmtOrderSensitiveReasons,
}

impl StmtEvalFacts {
  #[inline]
  fn from_tree_shaking_flags(tree_shaking_flags: StmtEvalFlags) -> Self {
    Self { tree_shaking_flags, order_sensitive_reasons: StmtOrderSensitiveReasons::empty() }
  }

  #[inline]
  fn from_tree_shaking_side_effect(has_side_effect: bool) -> Self {
    Self::from_tree_shaking_flags(has_side_effect.into())
  }

  #[inline]
  fn from_unknown_side_effect() -> Self {
    Self::from_tree_shaking_flags(StmtEvalFlags::UnknownSideEffect)
  }

  #[inline]
  pub fn tree_shaking_flags(&self) -> StmtEvalFlags {
    self.tree_shaking_flags
  }

  #[inline]
  fn set_order_sensitive_reason(&mut self, reason: StmtOrderSensitiveReasons, value: bool) {
    self.order_sensitive_reasons.set(reason, value);
  }

  #[inline]
  fn has_order_sensitive_reason(&self, reason: StmtOrderSensitiveReasons) -> bool {
    self.order_sensitive_reasons.contains(reason)
  }

  #[inline]
  fn without_unknown_side_effect(mut self) -> Self {
    self.tree_shaking_flags.remove(StmtEvalFlags::UnknownSideEffect);
    self
  }

  #[inline]
  fn has_side_effect_for_tree_shaking(&self) -> bool {
    self.tree_shaking_flags.has_side_effect_for_tree_shaking()
  }

  #[inline]
  pub fn is_order_sensitive(&self) -> bool {
    // `UnknownSideEffect` is stored in the tree-shaking channel, but unknown runtime work is
    // inherently order-sensitive. `PureCjs` remains tree-shaking-only.
    self.tree_shaking_flags.contains(StmtEvalFlags::UnknownSideEffect)
      || !self.order_sensitive_reasons.is_empty()
  }
}

impl BitOr for StmtEvalFacts {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    Self {
      tree_shaking_flags: self.tree_shaking_flags | rhs.tree_shaking_flags,
      order_sensitive_reasons: self.order_sensitive_reasons | rhs.order_sensitive_reasons,
    }
  }
}

impl BitOrAssign for StmtEvalFacts {
  fn bitor_assign(&mut self, rhs: Self) {
    self.tree_shaking_flags |= rhs.tree_shaking_flags;
    self.order_sensitive_reasons |= rhs.order_sensitive_reasons;
  }
}

/// Collect facts about evaluating a statement.
///
/// The returned facts keep tree-shaking side effects separate from order-sensitive reasons.
pub struct StmtEvalAnalyzer<'a> {
  scope: &'a AstScopes,
  options: &'a SharedNormalizedBundlerOptions,
  flat_options: FlatOptions,
  /// Cross-module optimization: node IDs of call expressions to known-pure functions.
  side_effect_free_call_expr_node_ids: Option<&'a FxHashSet<NodeId>>,
  /// Symbol IDs of namespace imports (`import * as ns from '...'`).
  /// Property reads on ES module namespace objects are guaranteed side-effect-free
  /// because namespace objects are frozen/sealed by spec with no getters.
  namespace_object_symbol_ids: Option<&'a FxHashSet<SymbolId>>,
}

impl<'a> StmtEvalAnalyzer<'a> {
  pub fn new(
    scope: &'a AstScopes,
    flat_options: FlatOptions,
    options: &'a SharedNormalizedBundlerOptions,
    side_effect_free_call_expr_node_ids: Option<&'a FxHashSet<NodeId>>,
    namespace_object_symbol_ids: Option<&'a FxHashSet<SymbolId>>,
  ) -> Self {
    Self {
      scope,
      options,
      flat_options,
      side_effect_free_call_expr_node_ids,
      namespace_object_symbol_ids,
    }
  }

  /// Check if a call expression has been marked pure by cross-module optimization.
  fn is_call_expr_marked_pure(&self, expr: &CallExpression) -> bool {
    self.side_effect_free_call_expr_node_ids.is_some_and(|set| set.contains(&expr.node_id()))
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

  /// `import.meta.url` is a spec-defined side-effect-free property read, and
  /// `import.meta.ROLLUP_FILE_URL_<referenceId>` is a placeholder the finalizer rewrites into a
  /// `new URL(...)` expression. Other accesses like `import.meta.hot.accept()` may have side effects.
  fn is_side_effect_free_import_meta_access(member_expr: &ast::MemberExpression) -> bool {
    let Expression::MetaProperty(meta_property) = member_expr.object() else {
      return false;
    };
    if meta_property.meta.name != "import" || meta_property.property.name != "meta" {
      return false;
    }
    member_expr
      .static_property_name()
      .is_some_and(|name| name == "url" || name.starts_with("ROLLUP_FILE_URL_"))
  }

  fn analyze_member_expr(&self, member_expr: &ast::MemberExpression) -> StmtEvalFacts {
    if self.is_expr_manual_pure_functions(member_expr.object()) {
      return StmtEvalFacts::default();
    }
    // ES module namespace objects are frozen/sealed by spec — property reads
    // on them are guaranteed side-effect-free. A computed key is still evaluated,
    // though, and may have its own side effects (e.g. `ns[foo()]`).
    if self.is_namespace_member_access(member_expr) == Some(true) {
      return match member_expr {
        ast::MemberExpression::ComputedMemberExpression(e) => self.analyze_expr(&e.expression),
        _ => StmtEvalFacts::default(),
      };
    }
    if Self::is_side_effect_free_import_meta_access(member_expr) {
      return StmtEvalFacts::default();
    }
    let is_global = self.is_member_expr_root_global(member_expr);
    let has_side_effect = member_expr.may_have_side_effects(self);
    let mut facts = StmtEvalFacts::from_tree_shaking_side_effect(has_side_effect);
    facts.set_order_sensitive_reason(StmtOrderSensitiveReasons::GlobalVarAccess, is_global);
    facts
  }

  /// Analyze a member-like write target after Oxc has determined the full write is side-effect-free.
  /// Writing to a property of an unresolved global still mutates shared state for tree shaking.
  fn analyze_side_effect_free_write_target(
    &self,
    target: &ast::SimpleAssignmentTarget,
  ) -> StmtEvalFacts {
    let object_detail = match target {
      ast::SimpleAssignmentTarget::StaticMemberExpression(e) => self.analyze_expr(&e.object),
      ast::SimpleAssignmentTarget::ComputedMemberExpression(e) => {
        self.analyze_expr(&e.object) | self.analyze_expr(&e.expression)
      }
      ast::SimpleAssignmentTarget::PrivateFieldExpression(e) => self.analyze_expr(&e.object),
      _ => return StmtEvalFacts::default(),
    };
    if object_detail.has_order_sensitive_reason(StmtOrderSensitiveReasons::GlobalVarAccess) {
      object_detail | StmtEvalFacts::from_unknown_side_effect()
    } else {
      object_detail
    }
  }

  fn analyze_call_expr(&self, expr: &CallExpression) -> StmtEvalFacts {
    let is_pure_annotated =
      !self.flat_options.ignore_annotations() && (expr.pure || self.is_call_expr_marked_pure(expr));

    // For pure-annotated calls, the call itself is side-effect-free.
    // We must check args via Rolldown's analyzer (not Oxc's) because Rolldown
    // has bundler-specific overrides (e.g. import.meta.url is side-effect-free).
    // Oxc's pure-call handling would still check args via its own may_have_side_effects,
    // which doesn't know about these overrides.
    let has_side_effect = if is_pure_annotated { false } else { expr.may_have_side_effects(self) };

    let is_global_call = !has_side_effect
      && matches!(&expr.callee, Expression::Identifier(id) if self.is_unresolved_reference(id));

    let mut facts = StmtEvalFacts::from_tree_shaking_side_effect(has_side_effect);
    facts.set_order_sensitive_reason(StmtOrderSensitiveReasons::PureAnnotation, is_pure_annotated);
    facts.set_order_sensitive_reason(StmtOrderSensitiveReasons::GlobalVarAccess, is_global_call);

    if !has_side_effect {
      // The call itself is known pure/safe; keep the callee's order-sensitive reasons while
      // discarding its unknown side-effect marker.
      facts |= self.analyze_expr(&expr.callee).without_unknown_side_effect();

      if is_pure_annotated {
        // Pure-annotated calls bypass Oxc's arg checking, so we must check args
        // through Rolldown's analyzer which has bundler-specific overrides
        // (e.g. import.meta.url is side-effect-free).
        for arg in &expr.arguments {
          facts |= match arg {
            Argument::SpreadElement(_) => StmtEvalFacts::from_unknown_side_effect(),
            _ => self.analyze_expr(arg.to_expression()),
          };
          if facts.has_side_effect_for_tree_shaking() {
            break;
          }
        }
      } else {
        // Oxc already verified args are side-effect-free; keep their order-sensitive reasons while
        // discarding unknown side-effect markers from this side-effect-free call.
        for arg in &expr.arguments {
          if let Argument::SpreadElement(_) = arg {
            break;
          }
          facts |= self.analyze_expr(arg.to_expression()).without_unknown_side_effect();
        }
      }
    }
    facts
  }

  fn is_expr_manual_pure_functions(&self, expr: &Expression) -> bool {
    if self.flat_options.is_manual_pure_functions_empty() {
      return false;
    }
    let manual_pure_functions = self.options.treeshake.manual_pure_functions().unwrap();
    extract_first_part_of_member_expr_like(expr)
      .is_some_and(|first| manual_pure_functions.contains(first))
  }

  fn analyze_expr(&self, expr: &Expression) -> StmtEvalFacts {
    // Peel transparent wrappers (`(x)`, `x as T`, `x satisfies T`, `x!`, `<T>x`, `x<T>`)
    // — they add no runtime semantics, so we recurse into the inner expression.
    let expr = expr.get_inner_expression();
    match expr {
      // --- Bundler-specific overrides (order-sensitive reasons or custom logic) ---
      oxc::ast::match_member_expression!(Expression) => {
        self.analyze_member_expr(expr.to_member_expression())
      }
      Expression::Identifier(ident) => self.analyze_identifier(ident),
      Expression::AssignmentExpression(assign_expr) => {
        // Bundler-specific: CJS `exports.foo = ...` must be checked before Oxc,
        // because Oxc would see a write to an unresolved global and return true.
        if let Some(pure_cjs) = check_pure_cjs_export(self.scope, &assign_expr.left) {
          return StmtEvalFacts::from_tree_shaking_flags(pure_cjs)
            | self.analyze_expr(&assign_expr.right);
        }
        if assign_expr.may_have_side_effects(self) {
          return StmtEvalFacts::from_unknown_side_effect();
        }
        self.analyze_side_effect_free_write_target(assign_expr.left.to_simple_assignment_target())
      }

      Expression::ChainExpression(chain_expr) => match &chain_expr.expression {
        ChainElement::CallExpression(call_expr) => self.analyze_call_expr(call_expr),
        ChainElement::TSNonNullExpression(ts_expr) => self.analyze_expr(&ts_expr.expression),
        match_member_expression!(ChainElement) => {
          self.analyze_member_expr(chain_expr.expression.to_member_expression())
        }
      },
      Expression::UpdateExpression(update_expr) => {
        if update_expr.may_have_side_effects(self) {
          return StmtEvalFacts::from_unknown_side_effect();
        }
        self.analyze_side_effect_free_write_target(&update_expr.argument)
      }
      Expression::NewExpression(expr) => {
        let has_side_effect = expr.may_have_side_effects(self);

        // Order sensitivity: constructor is a known global
        let is_global_constructor = !has_side_effect
          && matches!(&expr.callee, Expression::Identifier(id) if self.is_unresolved_reference(id));
        // Order sensitivity: marked with /*@__PURE__*/
        let is_pure_annotated = !self.flat_options.ignore_annotations() && expr.pure;

        let mut facts = StmtEvalFacts::from_tree_shaking_side_effect(has_side_effect);
        facts.set_order_sensitive_reason(
          StmtOrderSensitiveReasons::GlobalVarAccess,
          is_global_constructor,
        );
        facts
          .set_order_sensitive_reason(StmtOrderSensitiveReasons::PureAnnotation, is_pure_annotated);

        if !has_side_effect {
          // Oxc already verified args are side-effect-free; keep their order-sensitive reasons while
          // discarding unknown side-effect markers from this side-effect-free constructor.
          for arg in &expr.arguments {
            if let Argument::SpreadElement(_) = arg {
              break;
            }
            facts |= self.analyze_expr(arg.to_expression()).without_unknown_side_effect();
          }
        }
        facts
      }
      Expression::CallExpression(expr) => self.analyze_call_expr(expr),

      // Transparent wrappers: oxc validates the boolean side-effect status,
      // we then recurse children to collect order-sensitive reasons
      // (`PureAnnotation`, `GlobalVarAccess`). Without this, a pure-annotated
      // call buried inside `export default { foo: /* @__PURE__ */ ... }`
      // loses its annotation at the catch-all and the bundler emits the
      // module's var-init at top level rather than wrapping it.
      // See rolldown/rolldown#9425.
      Expression::ObjectExpression(obj) => self.fold_compound(
        expr,
        // `p.computed == true` => `p.key` is always an Expression variant
        // (oxc's parser only emits Static/PrivateIdentifier for `key:` syntax).
        obj
          .properties
          .iter()
          .flat_map(|prop| match prop {
            ast::ObjectPropertyKind::ObjectProperty(p) => {
              [p.computed.then(|| p.key.to_expression()), Some(&p.value)]
            }
            ast::ObjectPropertyKind::SpreadProperty(s) => [Some(&s.argument), None],
          })
          .flatten(),
      ),
      Expression::ArrayExpression(arr) => self.fold_compound(
        expr,
        arr.elements.iter().filter_map(|elem| match elem {
          ast::ArrayExpressionElement::SpreadElement(s) => Some(&s.argument),
          ast::ArrayExpressionElement::Elision(_) => None,
          e => Some(e.to_expression()),
        }),
      ),
      Expression::SequenceExpression(s) => self.fold_compound(expr, &s.expressions),
      Expression::TemplateLiteral(t) => self.fold_compound(expr, &t.expressions),
      Expression::ConditionalExpression(c) => {
        self.fold_compound(expr, [&c.test, &c.consequent, &c.alternate])
      }
      Expression::LogicalExpression(e) => self.fold_compound(expr, [&e.left, &e.right]),
      Expression::BinaryExpression(e) => self.fold_compound(expr, [&e.left, &e.right]),
      Expression::UnaryExpression(e) => self.fold_compound(expr, [&e.argument]),

      // Class expressions: same definition-time order-sensitivity as class declarations (heritage,
      // computed keys, static initializers, static blocks, decorators). Method/instance bodies are
      // not evaluated here.
      Expression::ClassExpression(class) => {
        self.analyze_class_definition(expr.may_have_side_effects(self), class)
      }

      // Everything else: delegate entirely to Oxc.
      // Covers literals, function/arrow expressions (bodies not evaluated
      // here), await/import/yield (inherently side-effectful), tagged-template
      // (handled like a call by oxc; no `pure` flag), JSX, V8 intrinsics, etc.
      _ => StmtEvalFacts::from_tree_shaking_side_effect(expr.may_have_side_effects(self)),
    }
  }

  /// Gate-then-fold helper for transparent compound expressions: ask oxc
  /// whether the whole expression has any side effect (fast path, covers
  /// computed keys / spread reads / etc.), and if not, fold order-sensitive reasons
  /// (`PureAnnotation`, `GlobalVarAccess`) from the child expressions.
  ///
  /// Removing `UnknownSideEffect` is load-bearing: rolldown's analyzer can be
  /// more conservative than oxc for some sub-expressions (e.g. CJS-export
  /// assignments via [`check_pure_cjs_export`], or future bundler-specific
  /// overrides). Once oxc has certified the parent side-effect-free at the
  /// gate, we trust that judgment for the parent and strip any `UnknownSideEffect`
  /// that a child contributed; order-sensitive reasons should still propagate.
  fn fold_compound<'e>(
    &self,
    expr: &Expression,
    children: impl IntoIterator<Item = &'e Expression<'e>>,
  ) -> StmtEvalFacts {
    if expr.may_have_side_effects(self) {
      return StmtEvalFacts::from_unknown_side_effect();
    }
    let mut facts = StmtEvalFacts::default();
    for child in children {
      facts |= self.analyze_expr(child);
      if facts.has_side_effect_for_tree_shaking() {
        break;
      }
    }
    facts.without_unknown_side_effect()
  }

  fn analyze_var_decl(&self, var_decl: &ast::VariableDeclaration) -> StmtEvalFacts {
    match var_decl.kind {
      VariableDeclarationKind::AwaitUsing => StmtEvalFacts::from_unknown_side_effect(),
      VariableDeclarationKind::Using => self.analyze_using_declarators(&var_decl.declarations),
      _ => {
        let mut facts = StmtEvalFacts::default();
        for declarator in &var_decl.declarations {
          facts |= match &declarator.id {
            BindingPattern::ObjectPattern(_) if self.flat_options.property_read_side_effects() => {
              StmtEvalFacts::from_unknown_side_effect()
            }
            BindingPattern::ArrayPattern(pat)
              if pat.elements.iter().any(|p| {
                p.as_ref().is_some_and(|pat| !matches!(pat, BindingPattern::BindingIdentifier(_)))
              }) =>
            {
              StmtEvalFacts::from_unknown_side_effect()
            }
            _ => declarator.init.as_ref().map(|init| self.analyze_expr(init)).unwrap_or_default(),
          };
        }
        facts
      }
    }
  }

  /// Base facts for a `class` declaration/expression, plus its definition-time order-sensitive
  /// reasons. `has_side_effect` must be the caller's existing whole-class
  /// `may_have_side_effects` judgment so the tree-shaking channel stays byte-identical to what the
  /// delegate arms produced before; only when oxc certified the class side-effect-free do we look
  /// deeper, and then only to grow `order_sensitive_reasons`.
  fn analyze_class_definition(&self, has_side_effect: bool, class: &ast::Class) -> StmtEvalFacts {
    let mut facts = StmtEvalFacts::from_tree_shaking_side_effect(has_side_effect);
    if !has_side_effect {
      facts.order_sensitive_reasons |= self.collect_class_definition_time_reasons(class);
    }
    facts
  }

  /// Order-sensitive reasons (`GlobalVarAccess` / `PureAnnotation`) from a class's
  /// *definition-time-evaluated* positions — the expressions JS runs when the `class`
  /// declaration/expression itself is evaluated, as opposed to at construction or call time:
  /// class/element decorators, the heritage (`extends`) expression, computed member keys, static
  /// field/accessor initializers, and static blocks. Instance (non-static) field initializers run
  /// in the constructor and method/accessor bodies run when invoked, so neither is included.
  ///
  /// Callers invoke this only after oxc certified the class side-effect-free, so every position
  /// here is already free of tree-shaking side effects; `analyze_expr`/`analyze_stmt` are used
  /// purely to harvest their reasons — this returns reasons only, so any conservative tree-shaking
  /// flag a child analysis adds is discarded rather than leaking into the tree-shaking channel.
  fn collect_class_definition_time_reasons(&self, class: &ast::Class) -> StmtOrderSensitiveReasons {
    let mut reasons = StmtOrderSensitiveReasons::empty();

    // Class decorators and the heritage expression are evaluated when the class is defined.
    for decorator in &class.decorators {
      reasons |= self.analyze_expr(&decorator.expression).order_sensitive_reasons;
    }
    if let Some(super_class) = &class.super_class {
      reasons |= self.analyze_expr(super_class).order_sensitive_reasons;
    }

    for element in &class.body.body {
      match element {
        // Static block bodies run at definition time; route each statement through the statement
        // analyzer to collect its order-sensitive reasons precisely.
        ast::ClassElement::StaticBlock(block) => {
          for stmt in &block.body {
            reasons |= self.analyze_stmt(stmt).order_sensitive_reasons;
          }
        }
        ast::ClassElement::MethodDefinition(method) => {
          for decorator in &method.decorators {
            reasons |= self.analyze_expr(&decorator.expression).order_sensitive_reasons;
          }
          if method.computed {
            reasons |= self.analyze_expr(method.key.to_expression()).order_sensitive_reasons;
          }
          // Method/getter/setter bodies run when invoked, not at definition time.
        }
        ast::ClassElement::PropertyDefinition(prop) => {
          for decorator in &prop.decorators {
            reasons |= self.analyze_expr(&decorator.expression).order_sensitive_reasons;
          }
          if prop.computed {
            reasons |= self.analyze_expr(prop.key.to_expression()).order_sensitive_reasons;
          }
          // Static initializers run at definition time; instance initializers run in the
          // constructor, so only static ones are definition-time order-sensitive.
          if prop.r#static
            && let Some(value) = &prop.value
          {
            reasons |= self.analyze_expr(value).order_sensitive_reasons;
          }
        }
        ast::ClassElement::AccessorProperty(accessor) => {
          for decorator in &accessor.decorators {
            reasons |= self.analyze_expr(&decorator.expression).order_sensitive_reasons;
          }
          if accessor.computed {
            reasons |= self.analyze_expr(accessor.key.to_expression()).order_sensitive_reasons;
          }
          if accessor.r#static
            && let Some(value) = &accessor.value
          {
            reasons |= self.analyze_expr(value).order_sensitive_reasons;
          }
        }
        // Type-level construct, no runtime evaluation.
        ast::ClassElement::TSIndexSignature(_) => {}
      }
    }

    reasons
  }

  fn analyze_decl(&self, decl: &ast::Declaration) -> StmtEvalFacts {
    match decl {
      ast::Declaration::VariableDeclaration(var_decl) => self.analyze_var_decl(var_decl),
      // Class definition-time positions (heritage, computed keys, static initializers, static
      // blocks, decorators) can read a whitelisted global or carry a pure annotation, which makes
      // the class order-sensitive even when oxc reports no tree-shaking side effect. The delegate
      // catch-all below drops those reasons.
      ast::Declaration::ClassDeclaration(class) => {
        self.analyze_class_definition(decl.may_have_side_effects(self), class)
      }
      _ => StmtEvalFacts::from_tree_shaking_side_effect(decl.may_have_side_effects(self)),
    }
  }

  fn analyze_using_declarators(&self, declarators: &[ast::VariableDeclarator]) -> StmtEvalFacts {
    let mut facts = StmtEvalFacts::default();
    for decl in declarators {
      facts |= decl
        .init
        .as_ref()
        .map(|init| match init {
          Expression::NullLiteral(_) => StmtEvalFacts::default(),
          // Side effect detection of identifier is different with other position when as initialization of using declaration.
          // Global variable `undefined` is considered as side effect free.
          Expression::Identifier(id) => StmtEvalFacts::from_tree_shaking_side_effect(
            !(id.name == "undefined" && self.is_unresolved_reference(id)),
          ),
          Expression::UnaryExpression(expr) if matches!(expr.operator, UnaryOperator::Void) => {
            self.analyze_expr(&expr.argument)
          }
          _ => StmtEvalFacts::from_unknown_side_effect(),
        })
        .unwrap_or_default();
      if facts.has_side_effect_for_tree_shaking() {
        break;
      }
    }
    facts
  }

  fn analyze_identifier(&self, ident_ref: &IdentifierReference) -> StmtEvalFacts {
    let is_global = self.is_unresolved_reference(ident_ref);
    // Delegate side-effect bool to Oxc (checks known globals, unknown_global_side_effects)
    let has_side_effect = ident_ref.may_have_side_effects(self);
    let mut facts = StmtEvalFacts::from_tree_shaking_side_effect(has_side_effect);
    facts.set_order_sensitive_reason(StmtOrderSensitiveReasons::GlobalVarAccess, is_global);
    facts
  }

  /// Bundler-specific: module declarations like import/export are treated
  /// differently than in generic JS analysis.
  /// - import/export-all/re-export: side-effect-free (bundler handles them)
  /// - export default: recurse into declaration
  /// - export named with source: side-effect-free
  fn analyze_module_declaration(&self, decl: &ast::ModuleDeclaration) -> StmtEvalFacts {
    match decl {
      ast::ModuleDeclaration::ExportAllDeclaration(_)
      | ast::ModuleDeclaration::ImportDeclaration(_) => {
        // We consider `import ...` has no side effect. However, `import ...` might be rewritten to other statements by the bundler.
        // In that case, we will mark the statement as having side effect in link stage.
        StmtEvalFacts::default()
      }
      ast::ModuleDeclaration::ExportDefaultDeclaration(default_decl) => {
        use oxc::ast::ast::ExportDefaultDeclarationKind;
        match &default_decl.declaration {
          decl @ oxc::ast::match_expression!(ExportDefaultDeclarationKind) => {
            self.analyze_expr(decl.to_expression())
          }
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(_) => StmtEvalFacts::default(),
          ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            // Same definition-time order-sensitivity as a plain class declaration.
            self.analyze_class_definition(class.may_have_side_effects(self), class)
          }
          ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
            StmtEvalFacts::from_unknown_side_effect()
          }
        }
      }
      ast::ModuleDeclaration::ExportNamedDeclaration(named_decl) => {
        if named_decl.source.is_some() {
          StmtEvalFacts::default()
        } else {
          named_decl.declaration.as_ref().map(|decl| self.analyze_decl(decl)).unwrap_or_default()
        }
      }
      ast::ModuleDeclaration::TSExportAssignment(_)
      | ast::ModuleDeclaration::TSNamespaceExportDeclaration(_) => {
        StmtEvalFacts::from_unknown_side_effect()
      }
    }
  }

  pub fn analyze_stmt(&self, stmt: &ast::Statement) -> StmtEvalFacts {
    use oxc::ast::ast::Statement;
    match stmt {
      // Bundler-specific: module declarations
      oxc::ast::match_module_declaration!(Statement) => {
        self.analyze_module_declaration(stmt.to_module_declaration())
      }
      // Language-level: everything else
      oxc::ast::match_declaration!(Statement) => self.analyze_decl(stmt.to_declaration()),
      Statement::ExpressionStatement(expr) => self.analyze_expr(&expr.expression),
      Statement::BlockStatement(block) => self.analyze_block(block),
      Statement::DoWhileStatement(do_while) => {
        self.analyze_stmt(&do_while.body) | self.analyze_expr(&do_while.test)
      }
      Statement::WhileStatement(while_stmt) => {
        self.analyze_expr(&while_stmt.test) | self.analyze_stmt(&while_stmt.body)
      }
      Statement::IfStatement(if_stmt) => {
        self.analyze_expr(&if_stmt.test)
          | self.analyze_stmt(&if_stmt.consequent)
          | if_stmt.alternate.as_ref().map(|s| self.analyze_stmt(s)).unwrap_or_default()
      }
      Statement::ReturnStatement(ret_stmt) => {
        ret_stmt.argument.as_ref().map(|expr| self.analyze_expr(expr)).unwrap_or_default()
      }
      Statement::LabeledStatement(labeled_stmt) => self.analyze_stmt(&labeled_stmt.body),
      Statement::TryStatement(try_stmt) => {
        let mut facts = self.analyze_block(&try_stmt.block);
        facts |= try_stmt
          .handler
          .as_ref()
          .map(|handler| self.analyze_block(&handler.body))
          .unwrap_or_default();
        facts |= try_stmt
          .finalizer
          .as_ref()
          .map(|finalizer| self.analyze_block(finalizer))
          .unwrap_or_default();
        facts
      }
      Statement::SwitchStatement(switch_stmt) => {
        let mut facts = self.analyze_expr(&switch_stmt.discriminant);
        if facts.has_side_effect_for_tree_shaking() {
          return facts;
        }
        'outer: for case in &switch_stmt.cases {
          facts |= case.test.as_ref().map(|expr| self.analyze_expr(expr)).unwrap_or_default();
          for stmt in &case.consequent {
            facts |= self.analyze_stmt(stmt);
            if facts.has_side_effect_for_tree_shaking() {
              break 'outer;
            }
          }
        }
        facts
      }
      // Everything else: delegate to Oxc.
      // This covers Empty, Continue, Break, Debugger, For/ForIn/ForOf, Throw, With.
      _ => StmtEvalFacts::from_tree_shaking_side_effect(stmt.may_have_side_effects(self)),
    }
  }

  fn analyze_block(&self, block: &ast::BlockStatement) -> StmtEvalFacts {
    let mut facts = StmtEvalFacts::default();
    for stmt in &block.body {
      facts |= self.analyze_stmt(stmt);
      if facts.has_side_effect_for_tree_shaking() {
        break;
      }
    }
    facts
  }
}

/// Bundler-specific: detect `exports.staticProp = ...` CJS export pattern.
/// Returns `Some(PureCjs)` if the target matches, `None` otherwise.
fn check_pure_cjs_export(scope: &AstScopes, target: &AssignmentTarget) -> Option<StmtEvalFlags> {
  match target {
    AssignmentTarget::ComputedMemberExpression(_) | AssignmentTarget::StaticMemberExpression(_) => {
      let member_expr = target.to_member_expression();
      if let Expression::Identifier(ident) = member_expr.object() {
        if ident.name == "exports"
          && member_expr.static_property_name().is_some()
          && ident.reference_id.get().is_some_and(|ref_id| scope.is_unresolved(ref_id))
        {
          return Some(StmtEvalFlags::PureCjs);
        }
      }
      None
    }
    _ => None,
  }
}

/// Extract the first (leftmost) identifier name from a member expression chain.
/// Used by both `StmtEvalAnalyzer::is_expr_manual_pure_functions` and
/// `StmtEvalAnalyzer::manual_pure_functions`.
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

impl GlobalContext<'_> for StmtEvalAnalyzer<'_> {
  fn is_global_reference(&self, reference: &IdentifierReference<'_>) -> bool {
    self.is_unresolved_reference(reference)
  }
}

impl MayHaveSideEffectsContext<'_> for StmtEvalAnalyzer<'_> {
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
  use rolldown_common::{AstScopes, NormalizedBundlerOptions, StmtEvalFlags};
  use rolldown_ecmascript::{EcmaAst, EcmaCompiler};

  use super::StmtEvalAnalyzer;
  use rolldown_common::FlatOptions;

  fn has_side_effect_for_tree_shaking(code: &str) -> bool {
    let source_type = SourceType::tsx();
    let ast = EcmaCompiler::parse("<Noop>", code, source_type).unwrap();
    let semantic = EcmaAst::make_semantic(ast.program());
    let scoping = semantic.into_scoping();
    let ast_scopes = AstScopes::new(scoping);

    let options = Arc::new(NormalizedBundlerOptions::default());
    let flags = FlatOptions::from_shared_options(&options);
    ast.program().body.iter().any(|stmt| {
      StmtEvalAnalyzer::new(&ast_scopes, flags, &options, None, None)
        .analyze_stmt(stmt)
        .has_side_effect_for_tree_shaking()
    })
  }

  fn get_stmt_eval_flags(code: &str) -> Vec<StmtEvalFlags> {
    let source_type = SourceType::tsx();
    let ast = EcmaCompiler::parse("<Noop>", code, source_type).unwrap();
    let semantic = EcmaAst::make_semantic(ast.program());
    let scoping = semantic.into_scoping();
    let ast_scopes = AstScopes::new(scoping);

    let options = Arc::new(NormalizedBundlerOptions::default());
    let flags = FlatOptions::from_shared_options(&options);
    ast
      .program()
      .body
      .iter()
      .map(|stmt| {
        StmtEvalAnalyzer::new(&ast_scopes, flags, &options, None, None)
          .analyze_stmt(stmt)
          .tree_shaking_flags()
      })
      .collect_vec()
  }

  fn get_stmt_order_sensitivity(code: &str) -> Vec<bool> {
    let source_type = SourceType::tsx();
    let ast = EcmaCompiler::parse("<Noop>", code, source_type).unwrap();
    let semantic = EcmaAst::make_semantic(ast.program());
    let scoping = semantic.into_scoping();
    let ast_scopes = AstScopes::new(scoping);

    let options = Arc::new(NormalizedBundlerOptions::default());
    let flags = FlatOptions::from_shared_options(&options);
    ast
      .program()
      .body
      .iter()
      .map(|stmt| {
        StmtEvalAnalyzer::new(&ast_scopes, flags, &options, None, None)
          .analyze_stmt(stmt)
          .is_order_sensitive()
      })
      .collect_vec()
  }

  #[test]
  fn test_side_effect_for_tree_shaking() {
    assert!(!has_side_effect_for_tree_shaking("export { a }"));
    assert!(!has_side_effect_for_tree_shaking("const a = {}"));
    assert!(!has_side_effect_for_tree_shaking(
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
    assert!(!has_side_effect_for_tree_shaking("`hello`"));
    assert!(has_side_effect_for_tree_shaking("const foo = ''; `hello${foo}`"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("`hello${foo}`"));
    assert!(has_side_effect_for_tree_shaking("const foo = {}; `hello${foo.bar}`"));
    assert!(has_side_effect_for_tree_shaking("tag`hello`"));
  }

  #[test]
  fn test_logical_expression() {
    assert!(!has_side_effect_for_tree_shaking("true && false"));
    assert!(!has_side_effect_for_tree_shaking("null ?? true"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("true && bar"));
    assert!(has_side_effect_for_tree_shaking("foo ?? true"));
  }

  #[test]
  fn test_parenthesized_expression() {
    assert!(!has_side_effect_for_tree_shaking("(true)"));
    assert!(!has_side_effect_for_tree_shaking("(null)"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("(bar)"));
    assert!(has_side_effect_for_tree_shaking("(foo)"));
  }

  #[test]
  fn test_sequence_expression() {
    assert!(!has_side_effect_for_tree_shaking("true, false"));
    assert!(!has_side_effect_for_tree_shaking("null, true"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("true, bar"));
    assert!(has_side_effect_for_tree_shaking("foo, true"));
  }

  #[test]
  fn test_conditional_expression() {
    assert!(!has_side_effect_for_tree_shaking("true ? false : true"));
    assert!(!has_side_effect_for_tree_shaking("null ? true : false"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("true ? bar : true"));
    assert!(has_side_effect_for_tree_shaking("foo ? true : false"));
    assert!(has_side_effect_for_tree_shaking("true ? bar : true"));
  }

  #[test]
  fn test_block_statement() {
    assert!(!has_side_effect_for_tree_shaking("{ }"));
    assert!(!has_side_effect_for_tree_shaking("{ const a = 1; }"));
    assert!(!has_side_effect_for_tree_shaking("{ const a = 1; const b = 2; }"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("{ const a = 1; bar; }"));
  }

  #[test]
  fn test_do_while_statement() {
    assert!(!has_side_effect_for_tree_shaking("do { } while (true)"));
    assert!(!has_side_effect_for_tree_shaking("do { const a = 1; } while (true)"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("do { const a = 1; } while (bar)"));
    assert!(has_side_effect_for_tree_shaking("do { const a = 1; bar; } while (true)"));
    assert!(has_side_effect_for_tree_shaking("do { bar; } while (true)"));
  }

  #[test]
  fn test_while_statement() {
    assert!(!has_side_effect_for_tree_shaking("while (true) { }"));
    assert!(!has_side_effect_for_tree_shaking("while (true) { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("while (bar) { const a = 1; }"));
    assert!(has_side_effect_for_tree_shaking("while (true) { const a = 1; bar; }"));
    assert!(has_side_effect_for_tree_shaking("while (true) { bar; }"));
  }

  #[test]
  fn test_if_statement() {
    assert!(!has_side_effect_for_tree_shaking("if (true) { }"));
    assert!(!has_side_effect_for_tree_shaking("if (true) { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("if (bar) { const a = 1; }"));
    assert!(has_side_effect_for_tree_shaking("if (true) { const a = 1; bar; }"));
    assert!(has_side_effect_for_tree_shaking("if (true) { bar; }"));
  }

  #[test]
  fn test_empty_statement() {
    assert!(!has_side_effect_for_tree_shaking(";"));
    assert!(!has_side_effect_for_tree_shaking(";;"));
  }

  #[test]
  fn test_continue_statement() {
    assert!(!has_side_effect_for_tree_shaking("continue;"));
  }

  #[test]
  fn test_break_statement() {
    assert!(!has_side_effect_for_tree_shaking("break;"));
  }

  #[test]
  fn test_return_statement() {
    assert!(!has_side_effect_for_tree_shaking("return;"));
    assert!(!has_side_effect_for_tree_shaking("return 1;"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("return bar;"));
  }

  #[test]
  fn test_labeled_statement() {
    assert!(!has_side_effect_for_tree_shaking("label: { }"));
    assert!(!has_side_effect_for_tree_shaking("label: { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("label: { const a = 1; bar; }"));
    assert!(has_side_effect_for_tree_shaking("label: { bar; }"));
  }

  #[test]
  fn test_try_statement() {
    assert!(!has_side_effect_for_tree_shaking("try { } catch (e) { }"));
    assert!(!has_side_effect_for_tree_shaking("try { const a = 1; } catch (e) { }"));
    assert!(!has_side_effect_for_tree_shaking("try { } catch (e) { const a = 1; }"));
    assert!(!has_side_effect_for_tree_shaking("try { const a = 1; } catch (e) { const a = 1; }"));
    assert!(!has_side_effect_for_tree_shaking("try { const a = 1; } finally { }"));
    assert!(!has_side_effect_for_tree_shaking("try { } catch (e) { const a = 1; } finally { }"));
    assert!(!has_side_effect_for_tree_shaking("try { } catch (e) { } finally { const a = 1; }"));
    assert!(!has_side_effect_for_tree_shaking(
      "try { const a = 1; } catch (e) { const a = 1; } finally { const a = 1; }"
    ));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("try { const a = 1; bar; } catch (e) { }"));
    assert!(has_side_effect_for_tree_shaking("try { } catch (e) { const a = 1; bar; }"));
    assert!(has_side_effect_for_tree_shaking("try { } catch (e) { bar; }"));
    assert!(has_side_effect_for_tree_shaking("try { const a = 1; } catch (e) { bar; }"));
    assert!(has_side_effect_for_tree_shaking("try { bar; } finally { }"));
    assert!(has_side_effect_for_tree_shaking("try { } catch (e) { bar; } finally { }"));
    assert!(has_side_effect_for_tree_shaking("try { } catch (e) { } finally { bar; }"));
    assert!(has_side_effect_for_tree_shaking("try { bar; } catch (e) { bar; } finally { bar; }"));
  }

  #[test]
  fn test_switch_statement() {
    assert!(!has_side_effect_for_tree_shaking("switch (true) { }"));
    assert!(!has_side_effect_for_tree_shaking("switch (true) { case 1: break; }"));
    assert!(!has_side_effect_for_tree_shaking("switch (true) { case 1: break; default: break; }"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("switch (bar) { case 1: break; }"));
    assert!(has_side_effect_for_tree_shaking("switch (true) { case 1: bar; }"));
    assert!(has_side_effect_for_tree_shaking("switch (true) { case bar: break; }"));
    assert!(has_side_effect_for_tree_shaking("switch (true) { case 1: bar; default: bar; }"));
  }

  #[test]
  fn test_binary_expression() {
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("1 + foo"));
    assert!(has_side_effect_for_tree_shaking("2 + bar"));
    // Oxc correctly recognizes primitive literal operands as side-effect-free
    assert!(!has_side_effect_for_tree_shaking("1 + 1"));
    // Oxc doesn't do constant propagation through variables, so `a + b` is
    // conservatively treated as potentially side-effectful (ToPrimitive)
    assert!(has_side_effect_for_tree_shaking("const a = 1; const b = 2; a + b"));
  }

  #[test]
  fn test_private_in_expression() {
    // Oxc checks that the RHS is known to be an object; `this` and local
    // variables with unknown value type are conservatively treated as
    // potentially non-object, so `#x in this` / `#x in obj` may throw.
    assert!(has_side_effect_for_tree_shaking("#privateField in this"));
    assert!(has_side_effect_for_tree_shaking("const obj = {}; #privateField in obj"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("#privateField in bar"));
    assert!(has_side_effect_for_tree_shaking("#privateField in foo"));
  }

  #[test]
  fn test_this_expression() {
    // In oxc 0.125.0, `this` is now considered to potentially have side effects
    // because it may be used in contexts where accessing `this` could throw
    assert!(has_side_effect_for_tree_shaking("this"));
    assert!(has_side_effect_for_tree_shaking("this.a"));
    assert!(has_side_effect_for_tree_shaking("this.a + this.b"));
    assert!(has_side_effect_for_tree_shaking("this.a = 10"));
  }

  #[test]
  fn test_meta_property_expression() {
    assert!(!has_side_effect_for_tree_shaking("import.meta"));
    assert!(!has_side_effect_for_tree_shaking("const meta = import.meta"));
    assert!(!has_side_effect_for_tree_shaking("import.meta.url"));
    assert!(!has_side_effect_for_tree_shaking("import.meta?.url"));
    assert!(!has_side_effect_for_tree_shaking("import.meta['url']"));
    assert!(!has_side_effect_for_tree_shaking("import.meta?.['url']"));
    assert!(!has_side_effect_for_tree_shaking("import.meta.ROLLUP_FILE_URL_abc123"));
    assert!(!has_side_effect_for_tree_shaking("import.meta?.ROLLUP_FILE_URL_abc123"));
    assert!(!has_side_effect_for_tree_shaking("import.meta['ROLLUP_FILE_URL_abc123']"));
    assert!(!has_side_effect_for_tree_shaking("import.meta?.['ROLLUP_FILE_URL_abc123']"));
    assert!(has_side_effect_for_tree_shaking("import.meta[foo()]"));
    // Other import.meta properties are not spec-defined as side-effect-free
    assert!(has_side_effect_for_tree_shaking("import.meta.hot"));
    assert!(has_side_effect_for_tree_shaking("import.meta['hot']"));
    // Deeper chains may throw (e.g. import.meta.nonExisting is undefined, .foo throws TypeError)
    assert!(has_side_effect_for_tree_shaking("import.meta.nonExisting.foo"));
    assert!(has_side_effect_for_tree_shaking("const { url } = import.meta"));
    assert!(has_side_effect_for_tree_shaking("import.meta.url = 'test'"));
  }

  #[test]
  fn test_assignment_expression() {
    // Destructuring assignments are side-effectful (GetIterator / RequireObjectCoercible).
    assert!(has_side_effect_for_tree_shaking("let a; [] = a"));
    assert!(has_side_effect_for_tree_shaking("({} = a)"));
    assert!(has_side_effect_for_tree_shaking("let a; a = 1"));
    assert!(has_side_effect_for_tree_shaking("let a, b; a = b; a = b = 1"));
    // accessing global variable may have side effect
    assert!(has_side_effect_for_tree_shaking("b = 1"));
    assert!(has_side_effect_for_tree_shaking("[] = b"));
    assert!(has_side_effect_for_tree_shaking("let a; a = b"));
    assert!(has_side_effect_for_tree_shaking("let a; a.b = 1"));
    assert!(has_side_effect_for_tree_shaking("let a; a['b'] = 1"));
    assert!(has_side_effect_for_tree_shaking("let a; a = a.b"));
    assert!(has_side_effect_for_tree_shaking("let a, b; ({ a } = b)"));
    assert!(has_side_effect_for_tree_shaking("let a, b; ({ ...a } = b)"));
    assert!(has_side_effect_for_tree_shaking("let a, b; [ a ] = b"));
    assert!(has_side_effect_for_tree_shaking("let a, b; [ ...a ] = b"));
  }

  #[test]
  fn test_chain_expression() {
    assert!(!has_side_effect_for_tree_shaking("Object.create"));
    assert!(!has_side_effect_for_tree_shaking("Object?.create"));
    assert!(!has_side_effect_for_tree_shaking("let a; /*#__PURE__*/ a?.()"));
    assert!(has_side_effect_for_tree_shaking("let a; a?.b"));
    assert!(has_side_effect_for_tree_shaking("let a; a?.()"));
    assert!(has_side_effect_for_tree_shaking("let a; a?.[a]"));
  }

  #[test]
  fn test_other_statements() {
    assert!(has_side_effect_for_tree_shaking("debugger;"));
    assert!(has_side_effect_for_tree_shaking("for (const k in {}) { }"));
    assert!(has_side_effect_for_tree_shaking("let a; for (const v of []) { a++ }"));
    assert!(has_side_effect_for_tree_shaking("for (;;) { }"));
    assert!(has_side_effect_for_tree_shaking("throw 1;"));
    assert!(has_side_effect_for_tree_shaking("with(a) { }"));
    assert!(has_side_effect_for_tree_shaking("await 1"));
    assert!(has_side_effect_for_tree_shaking("import('foo')"));
    assert!(has_side_effect_for_tree_shaking("let a; a``"));
    assert!(has_side_effect_for_tree_shaking("let a; a++"));
  }

  #[test]
  fn test_new_expr() {
    assert!(!has_side_effect_for_tree_shaking("new Map()"));
    assert!(!has_side_effect_for_tree_shaking("new Set()"));
    assert!(!has_side_effect_for_tree_shaking("new Map([[1, 2], [3, 4]]);"));
    assert!(has_side_effect_for_tree_shaking("new Regex()"));
    assert!(!has_side_effect_for_tree_shaking(
      "new Date(); new Date(''); new Date(null); new Date(false); new Date(undefined)"
    ));

    // TypedArray constructors should be side-effect free with no args, null, or undefined
    assert!(!has_side_effect_for_tree_shaking("new Uint8Array()"));
    assert!(!has_side_effect_for_tree_shaking("new Uint8Array(null)"));
    assert!(!has_side_effect_for_tree_shaking("new Uint8Array(undefined)"));
    assert!(!has_side_effect_for_tree_shaking("new Int8Array()"));
    assert!(!has_side_effect_for_tree_shaking("new Uint16Array()"));
    assert!(!has_side_effect_for_tree_shaking("new Uint32Array()"));
    assert!(!has_side_effect_for_tree_shaking("new Float64Array()"));
    assert!(!has_side_effect_for_tree_shaking("new BigUint64Array()"));

    // TypedArray constructors with numeric args are side-effect free
    // (memory allocation is not an observable side effect for tree-shaking)
    assert!(!has_side_effect_for_tree_shaking("new Uint8Array(10)"));
    assert!(!has_side_effect_for_tree_shaking("new Int16Array(5)"));
    assert!(!has_side_effect_for_tree_shaking("new Int32Array(100)"));
    assert!(!has_side_effect_for_tree_shaking("new Float32Array(20)"));
    assert!(!has_side_effect_for_tree_shaking("new BigInt64Array(8)"));
    assert!(!has_side_effect_for_tree_shaking("new Uint8ClampedArray(256)"));

    // Symbol is not a constructor - using 'new' throws TypeError
    // All of these should have side effects (they throw errors)
    assert!(has_side_effect_for_tree_shaking("new Symbol()"));
    assert!(has_side_effect_for_tree_shaking("new Symbol('string')"));
    assert!(has_side_effect_for_tree_shaking("new Symbol(null)"));
    assert!(has_side_effect_for_tree_shaking("new Symbol(undefined)"));
    assert!(has_side_effect_for_tree_shaking("new Symbol({ toString() { throw new Error() } })"));
    assert!(has_side_effect_for_tree_shaking("let unknownVariable; new Symbol(unknownVariable)"));

    // Symbol() as a function call (without 'new') is side-effect-free with primitives
    assert!(!has_side_effect_for_tree_shaking("Symbol()"));
    assert!(!has_side_effect_for_tree_shaking("Symbol('string')"));
    assert!(!has_side_effect_for_tree_shaking("Symbol(null)"));
    assert!(!has_side_effect_for_tree_shaking("Symbol(undefined)"));
    assert!(!has_side_effect_for_tree_shaking("Symbol(123)"));
    assert!(!has_side_effect_for_tree_shaking("Symbol(true)"));

    // Symbol() with object argument has side effects (could call toString)
    assert!(has_side_effect_for_tree_shaking("Symbol({ toString() { throw new Error() } })"));

    // Symbol() with unknown variable has side effects (could be an object)
    assert!(has_side_effect_for_tree_shaking("let unknownVariable; Symbol(unknownVariable)"));

    // Test fallback logic for global constructors with primitive arguments
    // String, Number, Boolean, Object constructors are side-effect-free with primitives
    assert!(!has_side_effect_for_tree_shaking("new String()"));

    assert!(!has_side_effect_for_tree_shaking("new Number()"));

    assert!(!has_side_effect_for_tree_shaking("new Boolean()"));

    assert!(!has_side_effect_for_tree_shaking("new Object()"));

    assert!(has_side_effect_for_tree_shaking("new BigInt(123)"));
  }

  #[test]
  fn test_primitive_global_function_calls() {
    // String() - side-effect-free with primitive arguments only
    // Object conversion can call valueOf/toString with side effects
    assert!(!has_side_effect_for_tree_shaking("String()"));
    assert!(!has_side_effect_for_tree_shaking("String('hello')"));
    assert!(!has_side_effect_for_tree_shaking("String(123)"));
    assert!(!has_side_effect_for_tree_shaking("String(null)"));
    assert!(!has_side_effect_for_tree_shaking("String(undefined)"));
    assert!(!has_side_effect_for_tree_shaking("String(true)"));

    // String() with any value: Oxc's "coercion methods are pure" assumption
    // treats toString()/valueOf() as side-effect-free. String(Symbol()) is also
    // safe per spec (returns "Symbol()" without throwing).
    assert!(!has_side_effect_for_tree_shaking("String({})"));
    assert!(!has_side_effect_for_tree_shaking("String([1, 2, 3])"));
    assert!(!has_side_effect_for_tree_shaking("let obj; String(obj)"));

    // Number() - side-effect-free with primitive arguments only
    assert!(!has_side_effect_for_tree_shaking("Number()"));
    assert!(!has_side_effect_for_tree_shaking("Number('123')"));
    assert!(!has_side_effect_for_tree_shaking("Number(456)"));
    assert!(!has_side_effect_for_tree_shaking("Number(null)"));
    assert!(!has_side_effect_for_tree_shaking("Number(undefined)"));
    assert!(!has_side_effect_for_tree_shaking("Number(true)"));

    // Number() with object literals: Oxc checks ToPrimitive/ToNumeric.
    // {} has known valueOf/toString, so ToNumeric({}) = NaN (no throw).
    assert!(!has_side_effect_for_tree_shaking("Number({})"));
    assert!(has_side_effect_for_tree_shaking("let val; Number(val)"));

    // Boolean() - always side-effect free (no type conversion needed)
    assert!(!has_side_effect_for_tree_shaking("Boolean()"));
    assert!(!has_side_effect_for_tree_shaking("Boolean(true)"));
    assert!(!has_side_effect_for_tree_shaking("Boolean('text')"));
    assert!(!has_side_effect_for_tree_shaking("Boolean(0)"));
    assert!(!has_side_effect_for_tree_shaking("Boolean(null)"));
    assert!(!has_side_effect_for_tree_shaking("Boolean(undefined)"));

    // Boolean() with any value is side-effect free (just checks truthiness)
    assert!(!has_side_effect_for_tree_shaking("Boolean({})"));
    assert!(!has_side_effect_for_tree_shaking("let val; Boolean(val)"));

    // BigInt() - side-effect-free only with proven-safe arguments
    // BigInt() with no arguments throws TypeError
    assert!(has_side_effect_for_tree_shaking("BigInt()"));
    // Integer literals are safe
    assert!(!has_side_effect_for_tree_shaking("BigInt(123)"));
    assert!(!has_side_effect_for_tree_shaking("BigInt(0)"));
    assert!(!has_side_effect_for_tree_shaking("BigInt(-1)"));
    assert!(!has_side_effect_for_tree_shaking("BigInt(+1)"));
    // Boolean literals are safe
    assert!(!has_side_effect_for_tree_shaking("BigInt(true)"));
    assert!(!has_side_effect_for_tree_shaking("BigInt(false)"));
    // BigInt literals are safe
    assert!(!has_side_effect_for_tree_shaking("BigInt(123n)"));

    // BigInt() with strings: Oxc can statically validate integer strings.
    // BigInt("123") works, BigInt("abc") or BigInt("1.5") throws.
    assert!(!has_side_effect_for_tree_shaking("BigInt('456')"));
    assert!(has_side_effect_for_tree_shaking("BigInt('abc')"));

    // BigInt() with non-integer numbers throws RangeError
    assert!(has_side_effect_for_tree_shaking("BigInt(1.5)"));
    assert!(has_side_effect_for_tree_shaking("BigInt(NaN)"));
    assert!(has_side_effect_for_tree_shaking("BigInt(Infinity)"));
    assert!(has_side_effect_for_tree_shaking("BigInt(-Infinity)"));

    // BigInt() with undefined/null throws TypeError
    assert!(has_side_effect_for_tree_shaking("BigInt(undefined)"));
    assert!(has_side_effect_for_tree_shaking("BigInt(null)"));

    // BigInt() with unknown or object arguments has side effects
    assert!(has_side_effect_for_tree_shaking("let val; BigInt(val)"));
    assert!(has_side_effect_for_tree_shaking("BigInt({})"));

    // BigInt() with spread elements has side effects
    assert!(has_side_effect_for_tree_shaking("let args; BigInt(...args)"));

    // Spread elements should have side effects
    assert!(has_side_effect_for_tree_shaking("let args; String(...args)"));
    assert!(has_side_effect_for_tree_shaking("let args; Number(...args)"));
    assert!(has_side_effect_for_tree_shaking("let args; Boolean(...args)"));
  }

  #[test]
  fn test_regexp_constructor() {
    // RegExp() and new RegExp() with valid patterns/flags are side-effect-free
    // Valid patterns
    assert!(!has_side_effect_for_tree_shaking("RegExp()"));
    assert!(!has_side_effect_for_tree_shaking("new RegExp()"));
    assert!(!has_side_effect_for_tree_shaking("RegExp('abc')"));
    assert!(!has_side_effect_for_tree_shaking("new RegExp('abc')"));
    assert!(!has_side_effect_for_tree_shaking("RegExp('abc', 'g')"));
    assert!(!has_side_effect_for_tree_shaking("new RegExp('abc', 'g')"));
    assert!(!has_side_effect_for_tree_shaking("RegExp('abc', 'gi')"));
    assert!(!has_side_effect_for_tree_shaking("new RegExp('abc', 'gimsuy')"));
    // RegExp with a RegExp literal argument is valid
    assert!(!has_side_effect_for_tree_shaking("RegExp(/foo/)"));
    assert!(!has_side_effect_for_tree_shaking("new RegExp(/foo/)"));

    // Invalid patterns throw SyntaxError - these have side effects
    assert!(has_side_effect_for_tree_shaking("RegExp('[')"));
    assert!(has_side_effect_for_tree_shaking("new RegExp('[')"));
    assert!(has_side_effect_for_tree_shaking("RegExp('\\\\')"));
    assert!(has_side_effect_for_tree_shaking("new RegExp('\\\\')"));

    // Invalid flags throw SyntaxError - these have side effects
    assert!(has_side_effect_for_tree_shaking("RegExp('a', 'xyz')"));
    assert!(has_side_effect_for_tree_shaking("new RegExp('a', 'xyz')"));
    assert!(has_side_effect_for_tree_shaking("RegExp('a', 'gg')"));
    assert!(has_side_effect_for_tree_shaking("new RegExp('a', 'gg')"));

    // Non-literal arguments have side effects (can't statically validate)
    assert!(has_side_effect_for_tree_shaking("let p; RegExp(p)"));
    assert!(has_side_effect_for_tree_shaking("let p; new RegExp(p)"));
    assert!(has_side_effect_for_tree_shaking("let f; RegExp('a', f)"));
    assert!(has_side_effect_for_tree_shaking("let f; new RegExp('a', f)"));

    // RegExp literals are side-effect-free (they're validated at parse time)
    assert!(!has_side_effect_for_tree_shaking("/abc/"));
    assert!(!has_side_effect_for_tree_shaking("/abc/g"));
  }

  #[test]
  fn test_side_effects_of_global_variable_access() {
    assert!(!has_side_effect_for_tree_shaking("let a = undefined"));
    assert!(!has_side_effect_for_tree_shaking("let a = void 0"));
    assert!(!has_side_effect_for_tree_shaking("using undef_remove = void 0;"));
    assert!(has_side_effect_for_tree_shaking("using undef_keep = void test();"));
    assert!(!has_side_effect_for_tree_shaking("let a = NaN"));
    assert!(!has_side_effect_for_tree_shaking("let a = String"));
    assert!(!has_side_effect_for_tree_shaking("let a = Object.assign"));
    assert!(!has_side_effect_for_tree_shaking("let a = Object.prototype.propertyIsEnumerable"));
    assert!(!has_side_effect_for_tree_shaking("let a = Symbol.asyncDispose"));
    assert!(!has_side_effect_for_tree_shaking("let a = Math.E"));
    assert!(!has_side_effect_for_tree_shaking("let a = Reflect.apply"));
    assert!(!has_side_effect_for_tree_shaking("let a = JSON.stringify"));
    assert!(!has_side_effect_for_tree_shaking("let a = Proxy"));

    assert_eq!(
      get_stmt_eval_flags("let a = Proxy; let a = JSON.stringify"),
      vec![StmtEvalFlags::empty(), StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("let a = Proxy; let a = JSON.stringify"),
      vec![true, true]
    );
    // should have side effects other global member expr access
    assert!(has_side_effect_for_tree_shaking("let a = Object.test"));
    assert!(has_side_effect_for_tree_shaking("let a = Object.prototype.two"));
    assert!(has_side_effect_for_tree_shaking("let a = Reflect.something"));

    assert_eq!(
      get_stmt_eval_flags("let a = Reflect.something"),
      vec![StmtEvalFlags::UnknownSideEffect]
    );
    assert_eq!(get_stmt_order_sensitivity("let a = Reflect.something"), vec![true]);

    // sideEffectful Global variable access with pure annotation
    assert_eq!(
      get_stmt_eval_flags("let a = /*@__PURE__ */ Reflect.something()"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("let a = /*@__PURE__ */ Reflect.something()"),
      vec![true]
    );
  }

  #[test]
  fn test_object_expression() {
    assert!(!has_side_effect_for_tree_shaking("const of = { [1]: 'hi'}"));
    assert!(!has_side_effect_for_tree_shaking("const of = { [-1]: 'hi'}"));
    assert!(!has_side_effect_for_tree_shaking("const of = { [+1]: 'hi'}"));
    assert!(!has_side_effect_for_tree_shaking("let remove = { [void 0]: 'x' };"));
    assert!(has_side_effect_for_tree_shaking("let keep = { [void test()]: 'x' };"));
    // Oxc is more permissive about computed property keys (ignores ToPrimitive side effects).
    // `{}` has a known toString(), so Oxc considers this side-effect-free.
    assert!(!has_side_effect_for_tree_shaking("const of = { [{}]: 'hi'}"));
  }

  // https://github.com/rolldown/rolldown/issues/9425
  // PureAnnotation / GlobalVarAccess on a nested call must propagate up through
  // transparent compound expressions (Object, Array, Sequence, Conditional,
  // Logical, Binary, Unary, Template, TaggedTemplate, TS wrappers). Otherwise
  // a module whose only "side effect" is a pure-annotated IIFE buried inside
  // `export default { ... }` is never marked ExecutionOrderSensitive and the
  // bundler emits its var-init at top level, before sibling modules that the
  // IIFE depends on (regression after oxc 0.131 stopped inlining pure IIFEs
  // in DCE mode).
  #[test]
  fn test_pure_annotation_propagates_through_compound_expr() {
    assert_eq!(
      get_stmt_eval_flags("export default { foo: /* @__PURE__ */ (() => globalValue)() }"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default { foo: /* @__PURE__ */ (() => globalValue)() }"),
      vec![true]
    );
    assert_eq!(
      get_stmt_eval_flags("export default [/* @__PURE__ */ (() => globalValue)()]"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default [/* @__PURE__ */ (() => globalValue)()]"),
      vec![true]
    );
    assert_eq!(
      get_stmt_eval_flags("export default (0, /* @__PURE__ */ (() => globalValue)())"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default (0, /* @__PURE__ */ (() => globalValue)())"),
      vec![true]
    );
    assert_eq!(
      get_stmt_eval_flags("export default true ? /* @__PURE__ */ (() => globalValue)() : null"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity(
        "export default true ? /* @__PURE__ */ (() => globalValue)() : null"
      ),
      vec![true]
    );
    assert_eq!(
      get_stmt_eval_flags("export default true && /* @__PURE__ */ (() => globalValue)()"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default true && /* @__PURE__ */ (() => globalValue)()"),
      vec![true]
    );
    // BinaryExpression `===` does not ToPrimitive-coerce, so oxc returns
    // no own side effect and the order-sensitive reason can propagate.
    assert_eq!(
      get_stmt_eval_flags("export default /* @__PURE__ */ (() => 2)() === 1"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default /* @__PURE__ */ (() => 2)() === 1"),
      vec![true]
    );
    // `typeof` never has side effects per spec.
    assert_eq!(
      get_stmt_eval_flags("export default typeof /* @__PURE__ */ (() => true)()"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default typeof /* @__PURE__ */ (() => true)()"),
      vec![true]
    );
    // Computed key with a pure-annotated call must propagate too.
    assert_eq!(
      get_stmt_eval_flags("export default { [/* @__PURE__ */ (() => 'k')()]: 1 }"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default { [/* @__PURE__ */ (() => 'k')()]: 1 }"),
      vec![true]
    );
    // GlobalVarAccess on a bare member-expr buried in a compound wrapper
    // must also propagate (same mechanism, different flag).
    assert_eq!(get_stmt_eval_flags("let a = { x: Proxy }"), vec![StmtEvalFlags::empty()]);
    assert_eq!(get_stmt_order_sensitivity("let a = { x: Proxy }"), vec![true]);
    assert_eq!(get_stmt_eval_flags("let a = [JSON.stringify]"), vec![StmtEvalFlags::empty()]);
    assert_eq!(get_stmt_order_sensitivity("let a = [JSON.stringify]"), vec![true]);
    // Nested compound: array inside object inside object — propagation must
    // walk through every layer.
    assert_eq!(
      get_stmt_eval_flags("export default { a: { b: [/* @__PURE__ */ (() => globalValue)()] } }"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity(
        "export default { a: { b: [/* @__PURE__ */ (() => globalValue)()] } }"
      ),
      vec![true]
    );
    // TS wrapper passthroughs (`SourceType::tsx()` in test helper).
    assert_eq!(
      get_stmt_eval_flags("export default (/* @__PURE__ */ (() => globalValue)() as string)"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity(
        "export default (/* @__PURE__ */ (() => globalValue)() as string)"
      ),
      vec![true]
    );
    assert_eq!(
      get_stmt_eval_flags(
        "export default (/* @__PURE__ */ (() => globalValue)() satisfies string)"
      ),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity(
        "export default (/* @__PURE__ */ (() => globalValue)() satisfies string)"
      ),
      vec![true]
    );
    assert_eq!(
      get_stmt_eval_flags("export default /* @__PURE__ */ (() => globalValue)()!"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default /* @__PURE__ */ (() => globalValue)()!"),
      vec![true]
    );
    // TSInstantiationExpression `f<T>` — must be peeled like the other TS
    // wrappers. (`<T>x` TSTypeAssertion is ambiguous with JSX under tsx()
    // and can't be expressed in this harness; `get_inner_expression()`
    // peels it identically in non-tsx contexts.)
    assert_eq!(
      get_stmt_eval_flags("export default /* @__PURE__ */ ((() => globalValue) as any)<string>()"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity(
        "export default /* @__PURE__ */ ((() => globalValue) as any)<string>()"
      ),
      vec![true]
    );
  }

  // Function bodies are not evaluated at the statement level — a pure
  // annotation inside an arrow/function body must NOT propagate up.
  #[test]
  fn test_pure_annotation_not_propagated_through_function_body() {
    assert_eq!(
      get_stmt_eval_flags("export default () => /* @__PURE__ */ pureCall()"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default () => /* @__PURE__ */ pureCall()"),
      vec![false]
    );
    assert_eq!(
      get_stmt_eval_flags("export default { foo: () => /* @__PURE__ */ pureCall() }"),
      vec![StmtEvalFlags::empty()]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default { foo: () => /* @__PURE__ */ pureCall() }"),
      vec![false]
    );
  }

  // A real side effect anywhere in a compound expression must surface as
  // Unknown — the order-sensitive reason propagation arms must not weaken this.
  #[test]
  fn test_compound_expr_side_effectful_operand_still_unknown() {
    assert!(has_side_effect_for_tree_shaking("let a = { foo: globalCall() }"));
    assert!(has_side_effect_for_tree_shaking("let a = [globalCall()]"));
    assert!(has_side_effect_for_tree_shaking("let a = (globalCall(), 1)"));
    assert!(has_side_effect_for_tree_shaking("let a = true ? globalCall() : null"));
  }

  #[test]
  fn test_cjs_pattern() {
    assert_eq!(
      get_stmt_eval_flags("Object.defineProperty(exports, \"__esModule\", { value: true })"),
      vec![StmtEvalFlags::UnknownSideEffect]
    );

    assert_eq!(
      get_stmt_eval_flags(
        r"
      exports.a = function test() {};
      exports['b'] = function () {
        console.log('b')
      };
      "
      ),
      vec![StmtEvalFlags::PureCjs, StmtEvalFlags::PureCjs]
    );

    assert_eq!(
      get_stmt_eval_flags("exports.a = global()"),
      vec![StmtEvalFlags::UnknownSideEffect | StmtEvalFlags::PureCjs]
    );

    assert_eq!(
      get_stmt_eval_flags("exports[test()] = true"),
      vec![StmtEvalFlags::UnknownSideEffect]
    );

    assert_eq!(
      get_stmt_eval_flags(
        r"
      let a = {};
      Object.defineProperty(a, '__esModule', { value: true });
      "
      ),
      vec![StmtEvalFlags::empty(), StmtEvalFlags::UnknownSideEffect]
    );
  }

  #[test]
  fn test_class_expr() {
    assert!(!has_side_effect_for_tree_shaking(
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
    assert!(has_side_effect_for_tree_shaking("function fn() {} @fn class Class {}"));
    assert!(has_side_effect_for_tree_shaking("function fn() {} var MyClass = @fn class {}"));
    assert!(has_side_effect_for_tree_shaking("function fn() {} class MyClass { @fn accessor x }"));
    assert!(has_side_effect_for_tree_shaking(
      "function fn() {} class MyClass { @fn static accessor x }"
    ));
    assert!(has_side_effect_for_tree_shaking("function fn() {} class MyClass { @fn method() {} }"));
    assert!(has_side_effect_for_tree_shaking("function fn() {} class MyClass { @fn field }"));
  }

  // #10104 follow-up: a class's definition-time-evaluated positions (heritage, computed keys,
  // static field/accessor initializers, static blocks, decorators) that read a whitelisted global
  // or carry a pure annotation make the class order-sensitive — the same treatment
  // `var x = Math.max(1, 2)` already gets. The three oxc-delegate arms (class declaration, class
  // expression, `export default class`) used to drop those reasons. The tree-shaking channel is
  // untouched: every side-effect-free class here keeps `StmtEvalFlags::empty()`.
  #[test]
  fn test_class_definition_time_order_sensitivity() {
    // Static field initializer reading a whitelisted global -> order-sensitive, and NOT a
    // tree-shaking side effect (so only the order-sensitive channel grew).
    assert_eq!(get_stmt_order_sensitivity("class C { static x = Math.max(1, 2) }"), vec![true]);
    assert_eq!(
      get_stmt_eval_flags("class C { static x = Math.max(1, 2) }"),
      vec![StmtEvalFlags::empty()]
    );
    // Static field initializer with a pure annotation (mirrors the object-literal case above).
    assert_eq!(
      get_stmt_order_sensitivity("class C { static x = /* @__PURE__ */ (() => globalValue)() }"),
      vec![true]
    );
    assert_eq!(
      get_stmt_eval_flags("class C { static x = /* @__PURE__ */ (() => globalValue)() }"),
      vec![StmtEvalFlags::empty()]
    );
    // Heritage (extends) reading a global.
    assert_eq!(get_stmt_order_sensitivity("class C extends SomeGlobal {}"), vec![true]);
    // Computed key evaluating a global read.
    assert_eq!(get_stmt_order_sensitivity("class C { [Math.max(1, 2)]() {} }"), vec![true]);
    assert_eq!(
      get_stmt_eval_flags("class C { [Math.max(1, 2)]() {} }"),
      vec![StmtEvalFlags::empty()]
    );
    // Static block reading a global.
    assert_eq!(get_stmt_order_sensitivity("class C { static { Math.max(1, 2) } }"), vec![true]);
    // Class expression and `export default class` route through the same machinery.
    assert_eq!(
      get_stmt_order_sensitivity("const C = class { static x = Math.max(1, 2) }"),
      vec![true]
    );
    assert_eq!(
      get_stmt_order_sensitivity("export default class { static x = Math.max(1, 2) }"),
      vec![true]
    );
    assert_eq!(
      get_stmt_eval_flags("export default class { static x = Math.max(1, 2) }"),
      vec![StmtEvalFlags::empty()]
    );
  }

  // Positions that run at construction/call time — not class definition time — stay
  // order-insensitive, so on-demand wrapping is not needlessly triggered.
  #[test]
  fn test_class_non_definition_time_stays_order_insensitive() {
    assert_eq!(get_stmt_order_sensitivity("class C {}"), vec![false]);
    // Instance (non-static) field initializer runs in the constructor.
    assert_eq!(get_stmt_order_sensitivity("class C { x = Math.max(1, 2) }"), vec![false]);
    assert_eq!(get_stmt_eval_flags("class C { x = Math.max(1, 2) }"), vec![StmtEvalFlags::empty()]);
    assert_eq!(get_stmt_order_sensitivity("const C = class { x = Math.max(1, 2) }"), vec![false]);
    assert_eq!(
      get_stmt_order_sensitivity("export default class { x = Math.max(1, 2) }"),
      vec![false]
    );
    // Method body runs when invoked, not at definition time.
    assert_eq!(
      get_stmt_order_sensitivity("class C { m() { return Math.max(1, 2) } }"),
      vec![false]
    );
  }

  #[test]
  fn test_extract_first_part_of_member_expr_like() {
    assert_eq!(extract_first_part_of_member_expr_like_helper("a.b"), "a");
    assert_eq!(extract_first_part_of_member_expr_like_helper("styled?.div()"), "styled");
    assert_eq!(extract_first_part_of_member_expr_like_helper("styled()"), "styled");
    assert_eq!(extract_first_part_of_member_expr_like_helper("styled().div"), "styled");
    assert_eq!(extract_first_part_of_member_expr_like_helper("styled()()"), "styled");
  }

  fn extract_first_part_of_member_expr_like_helper(code: &str) -> String {
    let allocator = oxc::allocator::Allocator::default();
    let parser = Parser::new(&allocator, code, SourceType::ts());
    let expr = parser.parse_expression().unwrap();
    super::extract_first_part_of_member_expr_like(&expr).unwrap().to_string()
  }
}
