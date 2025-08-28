use crate::ast_scanner::side_effect_detector::utils::{
  extract_member_expr_chain, is_primitive_literal,
};
use oxc::ast::ast::{
  self, Argument, ArrayExpressionElement, AssignmentTarget, BindingPatternKind, CallExpression,
  ChainElement, Expression, IdentifierReference, PropertyKey, UnaryOperator,
  VariableDeclarationKind,
};
use oxc::ast::{match_expression, match_member_expression};
use rolldown_common::{AstScopes, SharedNormalizedBundlerOptions, SideEffectDetail};
use rolldown_utils::global_reference::{
  is_global_ident_ref, is_side_effect_free_member_expr_of_len_three,
  is_side_effect_free_member_expr_of_len_two,
};
use utils::{
  can_change_strict_to_loose, is_side_effect_free_unbound_identifier_ref,
  maybe_side_effect_free_global_constructor,
};

use self::utils::{PrimitiveType, known_primitive_type};

mod utils;

/// Detect if a statement "may" have side effect.
pub struct SideEffectDetector<'a> {
  pub scope: &'a AstScopes,
  pub ignore_annotations: bool,
  pub jsx_preserve: bool,
  options: &'a SharedNormalizedBundlerOptions,
  is_manual_pure_functions_empty: bool,
}

impl<'a> SideEffectDetector<'a> {
  pub fn new(
    scope: &'a AstScopes,
    ignore_annotations: bool,
    jsx_preserve: bool,
    options: &'a SharedNormalizedBundlerOptions,
  ) -> Self {
    Self {
      scope,
      ignore_annotations,
      jsx_preserve,
      options,
      is_manual_pure_functions_empty: options.treeshake.manual_pure_functions().is_none(),
    }
  }

  fn is_unresolved_reference(&self, ident_ref: &IdentifierReference) -> bool {
    self.scope.is_unresolved(ident_ref.reference_id.get().unwrap())
  }

  fn detect_side_effect_of_property_key(
    &self,
    key: &PropertyKey,
    is_computed: bool,
  ) -> SideEffectDetail {
    match key {
      PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => false.into(),
      key @ oxc::ast::match_expression!(PropertyKey) => (is_computed && {
        let key_expr = key.to_expression();
        match key_expr {
          match_member_expression!(Expression) => {
            if let Some((ref_id, chain)) =
              extract_member_expr_chain(key_expr.to_member_expression(), 2)
            {
              !(chain == ["Symbol", "iterator"] && self.scope.is_unresolved(ref_id))
            } else {
              true
            }
          }
          _ => !is_primitive_literal(self.scope, key_expr),
        }
      })
      .into(),
    }
  }

  /// ref: https://github.com/evanw/esbuild/blob/360d47230813e67d0312ad754cad2b6ee09b151b/internal/js_ast/js_ast_helpers.go#L2298-L2393
  fn detect_side_effect_of_class(&self, cls: &ast::Class) -> SideEffectDetail {
    use oxc::ast::ast::ClassElement;
    if !cls.decorators.is_empty() {
      return true.into();
    }
    cls
      .body
      .body
      .iter()
      .any(|elm| match elm {
        ClassElement::StaticBlock(static_block) => static_block
          .body
          .iter()
          .any(|stmt| self.detect_side_effect_of_stmt(stmt).has_side_effect()),
        ClassElement::MethodDefinition(def) => {
          if !def.decorators.is_empty() {
            return true;
          }
          if self.detect_side_effect_of_property_key(&def.key, def.computed).has_side_effect() {
            return true;
          }

          def.value.params.items.iter().any(|item| !item.decorators.is_empty())
        }
        ClassElement::PropertyDefinition(def) => {
          if !def.decorators.is_empty() {
            return true;
          }
          if self.detect_side_effect_of_property_key(&def.key, def.computed).has_side_effect() {
            return true;
          }

          def.r#static
            && def
              .value
              .as_ref()
              .is_some_and(|init| self.detect_side_effect_of_expr(init).has_side_effect())
        }
        ClassElement::AccessorProperty(def) => {
          (match &def.key {
            PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => false,
            key @ oxc::ast::match_expression!(PropertyKey) => {
              self.detect_side_effect_of_expr(key.to_expression()).has_side_effect()
            }
          } || def
            .value
            .as_ref()
            .is_some_and(|init| self.detect_side_effect_of_expr(init).has_side_effect()))
        }
        ClassElement::TSIndexSignature(_) => unreachable!("ts should be transpiled"),
      })
      .into()
  }

  fn detect_side_effect_of_member_expr(&self, expr: &ast::MemberExpression) -> SideEffectDetail {
    if self.is_expr_manual_pure_functions(expr.object()) {
      return false.into();
    }

    let property_read_side_effects = matches!(
      self.options.treeshake.property_read_side_effects(),
      rolldown_common::PropertyReadSideEffects::Always
    );

    let mut side_effects_detail = SideEffectDetail::empty();
    let max_len = 3;
    let mut chains = vec![];
    let mut cur = match expr {
      ast::MemberExpression::ComputedMemberExpression(computed_expr) => {
        if let ast::Expression::StringLiteral(ref str) = computed_expr.expression {
          chains.push(str.value);
        } else {
          side_effects_detail |= self.detect_side_effect_of_expr(&computed_expr.expression);
        }
        &computed_expr.object
      }
      ast::MemberExpression::StaticMemberExpression(static_expr) => {
        chains.push(static_expr.property.name);
        &static_expr.object
      }
      ast::MemberExpression::PrivateFieldExpression(_) => return true.into(),
    };

    // extract_rest_member_expr_chain
    loop {
      match cur {
        ast::Expression::StaticMemberExpression(expr) => {
          cur = &expr.object;
          chains.push(expr.property.name);
        }
        ast::Expression::ComputedMemberExpression(computed_expr) => {
          if let ast::Expression::StringLiteral(ref str) = computed_expr.expression {
            chains.push(str.value);
          } else {
            side_effects_detail |= self.detect_side_effect_of_expr(&computed_expr.expression);
          }
          cur = &computed_expr.object;
        }
        ast::Expression::Identifier(ident_ref) => {
          chains.push(ident_ref.name);
          chains.reverse();
          side_effects_detail
            .set(SideEffectDetail::GlobalVarAccess, self.is_unresolved_reference(ident_ref));
          break;
        }
        _ => break,
      }
      if chains.len() >= max_len && property_read_side_effects {
        return true.into();
      }
    }

    if !property_read_side_effects {
      return side_effects_detail;
    }

    side_effects_detail |= (match chains.len() {
      2 => !is_side_effect_free_member_expr_of_len_two(&chains),
      3 => !is_side_effect_free_member_expr_of_len_three(&chains),
      _ => true,
    })
    .into();
    side_effects_detail
  }

  fn detect_side_effect_of_assignment_target(&self, expr: &AssignmentTarget) -> SideEffectDetail {
    match expr {
      AssignmentTarget::ComputedMemberExpression(_)
      | AssignmentTarget::StaticMemberExpression(_) => {
        let member_expr = expr.to_member_expression();
        match member_expr.object() {
          Expression::Identifier(ident) => {
            // - exports.a = ...;
            // - exports['a'] = ...;
            if self.is_unresolved_reference(ident)
              && ident.name == "exports"
              && member_expr.static_property_name().is_some()
            {
              SideEffectDetail::PureCjs
            } else {
              true.into()
            }
          }
          _ => true.into(),
        }
      }

      AssignmentTarget::AssignmentTargetIdentifier(_)
      | AssignmentTarget::PrivateFieldExpression(_) => true.into(),

      AssignmentTarget::TSAsExpression(_)
      | AssignmentTarget::TSSatisfiesExpression(_)
      | AssignmentTarget::TSNonNullExpression(_)
      | AssignmentTarget::TSTypeAssertion(_) => unreachable!(),

      AssignmentTarget::ArrayAssignmentTarget(array_pattern) => {
        (!array_pattern.elements.is_empty() || array_pattern.rest.is_some()).into()
      }
      AssignmentTarget::ObjectAssignmentTarget(object_pattern) => {
        (!object_pattern.properties.is_empty() || object_pattern.rest.is_some()).into()
      }
    }
  }

  fn detect_side_effect_of_call_expr(&self, expr: &CallExpression) -> SideEffectDetail {
    if self.is_expr_manual_pure_functions(&expr.callee) {
      return false.into();
    }

    // TODO: with cjs tree shaking remove this may cause some runtime behavior incorrect.
    // But marking `Object.defineProperty(exports, "__esModule", { value: true })` as has side effect may incraese bundle size a little.
    // if is_object_define_property_es_module(self.scope, expr).unwrap_or_default() {
    //   return StmtSideEffect::Unknown;
    // }

    let is_pure = !self.ignore_annotations && expr.pure;
    if is_pure {
      // Even it is pure, we also wants to know if the callee has access global var
      // But we need to ignore the `Unknown` flag, since it is already marked as `pure`.
      let mut detail = SideEffectDetail::PureAnnotation;
      detail |= self.detect_side_effect_of_expr(&expr.callee) - SideEffectDetail::Unknown;
      for arg in &expr.arguments {
        detail |= match arg {
          Argument::SpreadElement(_) => true.into(),
          _ => self.detect_side_effect_of_expr(arg.to_expression()),
        };
        if detail.has_side_effect() {
          break;
        }
      }
      detail
    } else {
      true.into()
    }
  }

  fn is_expr_manual_pure_functions(&self, expr: &'a Expression) -> bool {
    if self.is_manual_pure_functions_empty {
      return false;
    }
    // `is_manual_pure_functions_empty` is false, so `manual_pure_functions` is `Some`.
    let manual_pure_functions = self.options.treeshake.manual_pure_functions().unwrap();
    let Some(first_part) = Self::extract_first_part_of_member_expr_like(expr) else {
      return false;
    };
    manual_pure_functions.contains(first_part)
  }

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
          ChainElement::TSNonNullExpression(_) => unreachable!(),
          ChainElement::PrivateFieldExpression(_) => break None,
        },
        _ => break None,
      }
    }
  }

  #[allow(clippy::too_many_lines)]
  fn detect_side_effect_of_expr(&self, expr: &Expression) -> SideEffectDetail {
    match expr {
      Expression::BooleanLiteral(_)
      | Expression::NullLiteral(_)
      | Expression::NumericLiteral(_)
      | Expression::BigIntLiteral(_)
      | Expression::RegExpLiteral(_)
      | Expression::FunctionExpression(_)
      | Expression::ArrowFunctionExpression(_)
      | Expression::MetaProperty(_)
      | Expression::ThisExpression(_)
      | Expression::StringLiteral(_) => false.into(),
      Expression::ObjectExpression(obj_expr) => {
        let mut detail = SideEffectDetail::empty();
        for obj_prop in &obj_expr.properties {
          detail |= match obj_prop {
            ast::ObjectPropertyKind::ObjectProperty(prop) => {
              self.detect_side_effect_of_property_key(&prop.key, prop.computed)
                | self.detect_side_effect_of_expr(&prop.value)
            }
            ast::ObjectPropertyKind::SpreadProperty(_) => {
              // ...[expression] is considered as having side effect.
              // see crates/rolldown/tests/fixtures/rollup/object-spread-side-effect
              true.into()
            }
          };
          if detail.has_side_effect() {
            break;
          }
        }
        detail
      }
      // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2533-L2539
      Expression::UnaryExpression(unary_expr) => match unary_expr.operator {
        ast::UnaryOperator::Typeof if matches!(unary_expr.argument, Expression::Identifier(_)) => {
          false.into()
        }
        _ => self.detect_side_effect_of_expr(&unary_expr.argument),
      },
      oxc::ast::match_member_expression!(Expression) => {
        self.detect_side_effect_of_member_expr(expr.to_member_expression())
      }
      Expression::ClassExpression(cls) => self.detect_side_effect_of_class(cls),
      // Accessing global variables considered as side effect.
      Expression::Identifier(ident) => self.detect_side_effect_of_identifier(ident),
      // https://github.com/evanw/esbuild/blob/360d47230813e67d0312ad754cad2b6ee09b151b/internal/js_ast/js_ast_helpers.go#L2576-L2588
      Expression::TemplateLiteral(literal) => {
        let mut detail = SideEffectDetail::empty();
        for expr in &literal.expressions {
          // Primitive type detection is more strict and faster than side_effects detection of
          // `Expr`, put it first to fail fast.
          detail |= (known_primitive_type(self.scope, expr) == PrimitiveType::Unknown).into();
          detail |= self.detect_side_effect_of_expr(expr);
          if detail.has_side_effect() {
            break;
          }
        }
        detail
      }
      Expression::LogicalExpression(logic_expr) => match logic_expr.operator {
        ast::LogicalOperator::Or => {
          let lhs = self.detect_side_effect_of_expr(&logic_expr.left);
          let mut rhs = self.detect_side_effect_of_expr(&logic_expr.right);
          rhs.set(
            SideEffectDetail::Unknown,
            !is_side_effect_free_unbound_identifier_ref(
              self.scope,
              &logic_expr.right,
              &logic_expr.left,
              false,
            )
            .unwrap_or_default()
              && rhs.contains(SideEffectDetail::Unknown),
          );
          lhs | rhs
        }
        ast::LogicalOperator::And => {
          let lhs = self.detect_side_effect_of_expr(&logic_expr.left);
          let mut rhs = self.detect_side_effect_of_expr(&logic_expr.right);
          rhs.set(
            SideEffectDetail::Unknown,
            !is_side_effect_free_unbound_identifier_ref(
              self.scope,
              &logic_expr.right,
              &logic_expr.left,
              true,
            )
            .unwrap_or_default()
              && rhs.contains(SideEffectDetail::Unknown),
          );
          lhs | rhs
        }
        ast::LogicalOperator::Coalesce => {
          self.detect_side_effect_of_expr(&logic_expr.left)
            | self.detect_side_effect_of_expr(&logic_expr.right)
        }
      },
      Expression::ParenthesizedExpression(paren_expr) => {
        self.detect_side_effect_of_expr(&paren_expr.expression)
      }
      Expression::SequenceExpression(seq_expr) => {
        let mut detail = SideEffectDetail::empty();

        for expr in &seq_expr.expressions {
          detail |= self.detect_side_effect_of_expr(expr);
          if detail.has_side_effect() {
            break;
          }
        }
        detail
      }
      // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2460-L2463
      Expression::ConditionalExpression(cond_expr) => {
        let detail = self.detect_side_effect_of_expr(&cond_expr.test);
        let mut consequent_detail = self.detect_side_effect_of_expr(&cond_expr.consequent);
        consequent_detail.set(
          SideEffectDetail::Unknown,
          !is_side_effect_free_unbound_identifier_ref(
            self.scope,
            &cond_expr.consequent,
            &cond_expr.test,
            true,
          )
          .unwrap_or_default()
            && consequent_detail.contains(SideEffectDetail::Unknown),
        );
        let mut alternate_detail = self.detect_side_effect_of_expr(&cond_expr.alternate);
        alternate_detail.set(
          SideEffectDetail::Unknown,
          !is_side_effect_free_unbound_identifier_ref(
            self.scope,
            &cond_expr.alternate,
            &cond_expr.test,
            false,
          )
          .unwrap_or_default()
            && alternate_detail.contains(SideEffectDetail::Unknown),
        );
        detail | consequent_detail | alternate_detail
      }
      Expression::TSAsExpression(_)
      | Expression::TSSatisfiesExpression(_)
      | Expression::TSTypeAssertion(_)
      | Expression::TSNonNullExpression(_)
      | Expression::TSInstantiationExpression(_) => unreachable!("ts should be transpiled"),
      // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2541-L2574
      Expression::BinaryExpression(binary_expr) => {
        match binary_expr.operator {
          ast::BinaryOperator::StrictEquality | ast::BinaryOperator::StrictInequality => {
            self.detect_side_effect_of_expr(&binary_expr.left)
              | self.detect_side_effect_of_expr(&binary_expr.right)
          }
          // Special-case "<" and ">" with string, number, or bigint arguments
          ast::BinaryOperator::GreaterThan
          | ast::BinaryOperator::LessThan
          | ast::BinaryOperator::GreaterEqualThan
          | ast::BinaryOperator::LessEqualThan => {
            let lt = known_primitive_type(self.scope, &binary_expr.left);
            match lt {
              PrimitiveType::Number | PrimitiveType::String | PrimitiveType::BigInt => {
                SideEffectDetail::from(known_primitive_type(self.scope, &binary_expr.right) != lt)
                  | self.detect_side_effect_of_expr(&binary_expr.left)
                  | self.detect_side_effect_of_expr(&binary_expr.right)
              }
              _ => true.into(),
            }
          }

          // For "==" and "!=", pretend the operator was actually "===" or "!==". If
          // we know that we can convert it to "==" or "!=", then we can consider the
          // operator itself to have no side effects. This matters because our mangle
          // logic will convert "typeof x === 'object'" into "typeof x == 'object'"
          // and since "typeof x === 'object'" is considered to be side-effect free,
          // we must also consider "typeof x == 'object'" to be side-effect free.
          ast::BinaryOperator::Equality | ast::BinaryOperator::Inequality => {
            SideEffectDetail::from(!can_change_strict_to_loose(
              self.scope,
              &binary_expr.left,
              &binary_expr.right,
            )) | self.detect_side_effect_of_expr(&binary_expr.left)
              | self.detect_side_effect_of_expr(&binary_expr.right)
          }

          _ => true.into(),
        }
      }
      Expression::PrivateInExpression(private_in_expr) => {
        self.detect_side_effect_of_expr(&private_in_expr.right)
      }
      Expression::AssignmentExpression(expr) => {
        self.detect_side_effect_of_assignment_target(&expr.left)
          | self.detect_side_effect_of_expr(&expr.right)
      }

      Expression::ChainExpression(expr) => match &expr.expression {
        ChainElement::CallExpression(call_expr) => self.detect_side_effect_of_call_expr(call_expr),
        ChainElement::TSNonNullExpression(expr) => {
          self.detect_side_effect_of_expr(&expr.expression)
        }
        match_member_expression!(ChainElement) => {
          self.detect_side_effect_of_member_expr(expr.expression.to_member_expression())
        }
      },

      Expression::TaggedTemplateExpression(expr) => {
        (!self.is_expr_manual_pure_functions(&expr.tag)).into()
      }
      Expression::Super(_)
      | Expression::AwaitExpression(_)
      | Expression::ImportExpression(_)
      | Expression::UpdateExpression(_)
      | Expression::YieldExpression(_)
      | Expression::V8IntrinsicExpression(_) => true.into(),

      Expression::JSXElement(_) | Expression::JSXFragment(_) => {
        if self.jsx_preserve {
          return true.into();
        }
        unreachable!("jsx should be transpiled")
      }

      Expression::ArrayExpression(expr) => self.detect_side_effect_of_array_expr(expr),
      Expression::NewExpression(expr) => {
        let is_side_effect_free_global_constructor =
          maybe_side_effect_free_global_constructor(self.scope, expr);
        let is_pure = expr.pure || is_side_effect_free_global_constructor;

        let mut detail = SideEffectDetail::empty();
        detail.set(SideEffectDetail::GlobalVarAccess, is_side_effect_free_global_constructor);
        detail.set(SideEffectDetail::Unknown, !is_pure);
        detail.set(SideEffectDetail::PureAnnotation, expr.pure);

        for arg in &expr.arguments {
          detail |= match arg {
            Argument::SpreadElement(_) => true.into(),
            _ => self.detect_side_effect_of_expr(arg.to_expression()),
          };
          if detail.has_side_effect() {
            break;
          }
        }
        detail
      }
      Expression::CallExpression(expr) => self.detect_side_effect_of_call_expr(expr),
    }
  }

  fn detect_side_effect_of_array_expr(&self, expr: &ast::ArrayExpression<'_>) -> SideEffectDetail {
    let mut detail = SideEffectDetail::empty();
    for elem in &expr.elements {
      let cur = match elem {
        ArrayExpressionElement::SpreadElement(ele) => {
          // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2466-L2477
          // Spread of an inline array such as "[...[x]]" is side-effect free
          match &ele.argument {
            Expression::ArrayExpression(arr) => self.detect_side_effect_of_array_expr(arr),
            _ => return true.into(),
          }
        }
        ArrayExpressionElement::Elision(_) => false.into(),
        match_expression!(ArrayExpressionElement) => {
          self.detect_side_effect_of_expr(elem.to_expression())
        }
      };
      detail |= cur;
    }
    detail
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
          // Whether to destructure import.meta
          if let BindingPatternKind::ObjectPattern(ref obj_pat) = declarator.id.kind {
            if !obj_pat.properties.is_empty() {
              if let Some(Expression::MetaProperty(_)) = declarator.init {
                return true.into();
              }
            }
          }
          detail |=
            match &declarator.id.kind {
              // Destructuring the initializer has no side effects if the
              // initializer is an array, since we assume the iterator is then
              // the built-in side-effect free array iterator.
              BindingPatternKind::ObjectPattern(_) => {
                // Object destructuring only has side effects when property_read_side_effects is Always
                if matches!(
                  self.options.treeshake.property_read_side_effects(),
                  rolldown_common::PropertyReadSideEffects::Always
                ) {
                  true.into()
                } else {
                  declarator
                    .init
                    .as_ref()
                    .map(|init| self.detect_side_effect_of_expr(init))
                    .unwrap_or(false.into())
                }
              }
              BindingPatternKind::ArrayPattern(pat) => {
                for p in &pat.elements {
                  if p.as_ref().is_some_and(|pat| {
                    !matches!(pat.kind, BindingPatternKind::BindingIdentifier(_))
                  }) {
                    return true.into();
                  }
                }
                declarator
                  .init
                  .as_ref()
                  .map(|init| self.detect_side_effect_of_expr(init))
                  .unwrap_or(false.into())
              }
              BindingPatternKind::BindingIdentifier(_)
              | BindingPatternKind::AssignmentPattern(_) => declarator
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
    use oxc::ast::ast::Declaration;
    match decl {
      Declaration::VariableDeclaration(var_decl) => self.detect_side_effect_of_var_decl(var_decl),
      Declaration::FunctionDeclaration(_) => false.into(),
      Declaration::ClassDeclaration(cls_decl) => self.detect_side_effect_of_class(cls_decl),
      Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_)
      | Declaration::TSEnumDeclaration(_)
      | Declaration::TSModuleDeclaration(_)
      | Declaration::TSImportEqualsDeclaration(_) => unreachable!("ts should be transpiled"),
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

  #[inline]
  fn detect_side_effect_of_identifier(&self, ident_ref: &IdentifierReference) -> SideEffectDetail {
    let mut detail = SideEffectDetail::empty();
    detail.set(SideEffectDetail::GlobalVarAccess, self.is_unresolved_reference(ident_ref));
    if detail.contains(SideEffectDetail::GlobalVarAccess) {
      detail.set(
        SideEffectDetail::Unknown,
        detail.contains(SideEffectDetail::GlobalVarAccess)
          && self.options.treeshake.unknown_global_side_effects()
          && !is_global_ident_ref(&ident_ref.name),
      );
    }
    detail
  }

  #[allow(clippy::too_many_lines)]
  pub fn detect_side_effect_of_stmt(&self, stmt: &ast::Statement) -> SideEffectDetail {
    use oxc::ast::ast::Statement;
    match stmt {
      oxc::ast::match_declaration!(Statement) => {
        self.detect_side_effect_of_decl(stmt.to_declaration())
      }
      Statement::ExpressionStatement(expr) => self.detect_side_effect_of_expr(&expr.expression),
      oxc::ast::match_module_declaration!(Statement) => match stmt.to_module_declaration() {
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
              self.detect_side_effect_of_class(decl)
            }
            ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
              unreachable!("ts should be transpiled")
            }
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
        | ast::ModuleDeclaration::TSNamespaceExportDeclaration(_) => {
          unreachable!("ts should be transpiled")
        }
      },
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
            .map(|stmt| self.detect_side_effect_of_stmt(stmt))
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

          if detail.has_side_effect() {
            break;
          }
        }
        detail
      }

      Statement::EmptyStatement(_)
      | Statement::ContinueStatement(_)
      | Statement::BreakStatement(_) => false.into(),

      Statement::DebuggerStatement(_)
      | Statement::ForInStatement(_)
      | Statement::ForOfStatement(_)
      | Statement::ForStatement(_)
      | Statement::ThrowStatement(_)
      | Statement::WithStatement(_) => true.into(),
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

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use itertools::Itertools;
  use oxc::{parser::Parser, span::SourceType};
  use rolldown_common::{AstScopes, NormalizedBundlerOptions, SideEffectDetail};
  use rolldown_ecmascript::{EcmaAst, EcmaCompiler};

  use crate::ast_scanner::side_effect_detector::SideEffectDetector;

  fn get_statements_side_effect(code: &str) -> bool {
    let source_type = SourceType::tsx();
    let ast = EcmaCompiler::parse("<Noop>", code, source_type).unwrap();
    let semantic = EcmaAst::make_semantic(ast.program(), false);
    let scoping = semantic.into_scoping();
    let ast_scopes = AstScopes::new(scoping);

    ast.program().body.iter().any(|stmt| {
      SideEffectDetector::new(
        &ast_scopes,
        false,
        false,
        &Arc::new(NormalizedBundlerOptions::default()),
      )
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

    ast
      .program()
      .body
      .iter()
      .map(|stmt| {
        SideEffectDetector::new(
          &ast_scopes,
          false,
          false,
          &Arc::new(NormalizedBundlerOptions::default()),
        )
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
    // + will invoke valueOf, which may have side effect
    assert!(get_statements_side_effect("1 + 1"));
    assert!(get_statements_side_effect("const a = 1; const b = 2; a + b"));
  }

  #[test]
  fn test_private_in_expression() {
    assert!(!get_statements_side_effect("#privateField in this"));
    assert!(!get_statements_side_effect("const obj = {}; #privateField in obj"));
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
    assert!(get_statements_side_effect("import.meta.url"));
    assert!(get_statements_side_effect("const { url } = import.meta"));
    assert!(get_statements_side_effect("import.meta.url = 'test'"));
  }

  #[test]
  fn test_assignment_expression() {
    assert!(!get_statements_side_effect("let a; [] = a; ({} = a)"));
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
    assert!(get_statements_side_effect("const of = { [{}]: 'hi'}"));
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
