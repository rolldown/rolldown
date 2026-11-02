//! Structured builtin wrappers. Resolved option values are captured by the renderer once;
//! assembly only moves finalized statements and constructs the bounded wrapper syntax.

use oxc::{
  allocator::{Allocator, CloneIn, TakeIn, Vec as ArenaVec},
  ast::{
    ast::{
      Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, Expression,
      FormalParameter, FormalParameterKind, FormalParameters, Function, FunctionBody, FunctionType,
      IdentifierName, ObjectPropertyKind, PropertyKey, PropertyKind, Statement, Str,
    },
    builder::AstBuilder,
  },
  ast_visit::VisitMut,
  span::Span,
  syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator},
};
use rolldown_ecmascript_utils::StatementFactoryExt as _;

pub enum WrapperKind {
  Flat,
  Iife,
  Umd { paths: Vec<String> },
}

#[expect(
  clippy::struct_excessive_bools,
  reason = "independent output options captured once by the renderer"
)]
pub struct WrapperAstPlan {
  pub kind: WrapperKind,
  pub name: Option<String>,
  pub extend: bool,
  pub has_exports: bool,
  pub named_exports: bool,
  pub parameters: Vec<String>,
  pub globals: Vec<String>,
  pub es_module: bool,
  pub to_string_tag: bool,
  pub tail: String,
  pub outer_prefix: String,
  pub outer_suffix: String,
  pub hashbang: Option<String>,
  pub directives: Vec<String>,
  pub factory_return: Option<(rolldown_common::SymbolRef, bool)>,
}

struct Generated(Span);
impl VisitMut<'_> for Generated {
  fn visit_span(&mut self, span: &mut Span) {
    *span = self.0;
  }
}

struct Nodes<'a> {
  builder: AstBuilder<'a>,
  allocator: &'a Allocator,
  span: Span,
}

impl<'a> Nodes<'a> {
  fn id(&self, name: &str) -> Expression<'a> {
    Expression::new_identifier(self.span, Str::from_str_in(name, &self.builder), &self.builder)
  }
  fn string(&self, value: &str) -> Expression<'a> {
    Expression::new_string_literal(
      self.span,
      Str::from_str_in(value, &self.builder),
      None,
      &self.builder,
    )
  }
  fn object(&self) -> Expression<'a> {
    Expression::new_object_expression(self.span, [], &self.builder)
  }
  fn member(&self, object: Expression<'a>, name: &str) -> Expression<'a> {
    if oxc::syntax::identifier::is_identifier_name(name) {
      Expression::new_static_member_expression(
        self.span,
        object,
        IdentifierName::new(self.span, Str::from_str_in(name, &self.builder), &self.builder),
        false,
        &self.builder,
      )
    } else {
      Expression::new_computed_member_expression(
        self.span,
        object,
        self.string(name),
        false,
        &self.builder,
      )
    }
  }
  fn call(
    &self,
    callee: Expression<'a>,
    args: impl IntoIterator<Item = Expression<'a>>,
  ) -> Expression<'a> {
    Expression::new_call_expression(
      self.span,
      callee,
      None,
      ArenaVec::from_iter_in(args.into_iter().map(Argument::from), &self.builder),
      false,
      &self.builder,
    )
  }
  fn assign(&self, left: Expression<'a>, right: Expression<'a>) -> Expression<'a> {
    let target = match left {
      Expression::Identifier(id) => AssignmentTarget::AssignmentTargetIdentifier(id),
      Expression::StaticMemberExpression(member) => {
        AssignmentTarget::StaticMemberExpression(member)
      }
      Expression::ComputedMemberExpression(member) => {
        AssignmentTarget::ComputedMemberExpression(member)
      }
      _ => unreachable!("wrapper assignment target"),
    };
    Expression::new_assignment_expression(
      self.span,
      AssignmentOperator::Assign,
      target,
      right,
      &self.builder,
    )
  }
  fn logical(
    &self,
    left: Expression<'a>,
    operator: LogicalOperator,
    right: Expression<'a>,
  ) -> Expression<'a> {
    Expression::new_logical_expression(self.span, left, operator, right, &self.builder)
  }
  fn conditional(
    &self,
    test: Expression<'a>,
    yes: Expression<'a>,
    no: Expression<'a>,
  ) -> Expression<'a> {
    Expression::new_conditional_expression(self.span, test, yes, no, &self.builder)
  }
  fn expr_stmt(&self, expr: Expression<'a>) -> Statement<'a> {
    Statement::new_expression_statement(self.span, expr, &self.builder)
  }
  fn sequence(&self, expressions: impl IntoIterator<Item = Expression<'a>>) -> Expression<'a> {
    Expression::new_sequence_expression(
      self.span,
      ArenaVec::from_iter_in(expressions, &self.builder),
      &self.builder,
    )
  }
  fn function(&self, names: &[String], body: ArenaVec<'a, Statement<'a>>) -> Expression<'a> {
    let params = FormalParameters::boxed(
      self.span,
      FormalParameterKind::FormalParameter,
      ArenaVec::from_iter_in(
        names.iter().map(|name| {
          FormalParameter::new(
            self.span,
            [],
            BindingPattern::new_binding_identifier(
              self.span,
              Str::from_str_in(name, &self.builder),
              &self.builder,
            ),
            None,
            None,
            false,
            None,
            false,
            false,
            &self.builder,
          )
        }),
        &self.builder,
      ),
      None,
      &self.builder,
    );
    Expression::FunctionExpression(Function::boxed(
      self.span,
      FunctionType::FunctionExpression,
      None,
      false,
      false,
      false,
      None,
      None,
      params,
      None,
      Some(FunctionBody::boxed(self.span, [], body, &self.builder)),
      &self.builder,
    ))
  }
  fn typeof_is(&self, name: &str, value: &str, equal: bool) -> Expression<'a> {
    Expression::new_binary_expression(
      self.span,
      Expression::new_unary_expression(
        self.span,
        UnaryOperator::Typeof,
        self.id(name),
        &self.builder,
      ),
      if equal { BinaryOperator::StrictEquality } else { BinaryOperator::StrictInequality },
      self.string(value),
      &self.builder,
    )
  }
  fn namespace(&self, root: Expression<'a>, name: &str) -> (Vec<Expression<'a>>, Expression<'a>) {
    let mut initialization = Vec::new();
    let mut target = root;
    let mut parts = name.split('.').peekable();
    while let Some(part) = parts.next() {
      target = self.member(target, part);
      if parts.peek().is_some() {
        initialization.push(self.assign(
          target.clone_in(self.allocator),
          self.logical(target.clone_in(self.allocator), LogicalOperator::Or, self.object()),
        ));
      }
    }
    (initialization, target)
  }
  fn property(
    &self,
    key: PropertyKey<'a>,
    value: Expression<'a>,
    computed: bool,
  ) -> ObjectPropertyKind<'a> {
    ObjectPropertyKind::new_object_property(
      self.span,
      PropertyKind::Init,
      key,
      value,
      false,
      false,
      computed,
      &self.builder,
    )
  }
  fn value_object(&self, value: Expression<'a>) -> Expression<'a> {
    Expression::new_object_expression(
      self.span,
      [self.property(
        PropertyKey::new_static_identifier(self.span, "value", &self.builder),
        value,
        false,
      )],
      &self.builder,
    )
  }
  fn marker(&self, es_module: bool, tag: bool) -> Option<Statement<'a>> {
    let tag_key = || self.member(self.id("Symbol"), "toStringTag");
    let callee = self.member(
      self.id("Object"),
      if es_module && tag { "defineProperties" } else { "defineProperty" },
    );
    let args = if es_module && tag {
      let fields = [
        self.property(
          PropertyKey::new_static_identifier(self.span, "__esModule", &self.builder),
          self.value_object(Expression::new_boolean_literal(self.span, true, &self.builder)),
          false,
        ),
        self.property(PropertyKey::from(tag_key()), self.value_object(self.string("Module")), true),
      ];
      vec![self.id("exports"), Expression::new_object_expression(self.span, fields, &self.builder)]
    } else if es_module {
      vec![
        self.id("exports"),
        self.string("__esModule"),
        self.value_object(Expression::new_boolean_literal(self.span, true, &self.builder)),
      ]
    } else if tag {
      vec![self.id("exports"), tag_key(), self.value_object(self.string("Module"))]
    } else {
      return None;
    };
    Some(self.expr_stmt(self.call(callee, args)))
  }
}

impl WrapperAstPlan {
  pub fn factory_return(
    ctx: &crate::types::generator::GenerateContext<'_>,
  ) -> Option<(rolldown_common::SymbolRef, bool)> {
    let entry = ctx.chunk.entry_module(&ctx.link_output.module_table)?;
    let meta = &ctx.link_output.metas[entry.idx];
    if matches!(meta.wrap_kind(), rolldown_common::WrapKind::Cjs)
      && !matches!(ctx.chunk.output_exports, rolldown_common::OutputExports::Named)
    {
      return Some((meta.wrapper_ref.expect("CJS wrapper"), true));
    }
    if !matches!(ctx.chunk.output_exports, rolldown_common::OutputExports::Default) {
      return None;
    }
    ctx.render_export_items_index_vec[ctx.chunk_idx]
      .iter()
      .next()
      .map(|(symbol, _)| (*symbol, false))
  }

  /// Wrapper nodes borrow only the chunk arena. No callbacks or module parsing happen here.
  pub fn wrap<'a>(
    &self,
    allocator: &'a Allocator,
    span: Span,
    body: ArenaVec<'a, Statement<'a>>,
    ctx: &crate::types::generator::GenerateContext<'_>,
  ) -> rolldown_error::BuildResult<ArenaVec<'a, Statement<'a>>> {
    let nodes = Nodes { builder: AstBuilder::new(allocator), allocator, span };
    let mut factory_body = ArenaVec::new_in(&nodes.builder);
    factory_body.extend(nodes.marker(self.es_module, self.to_string_tag));
    factory_body.extend(body);
    if let Some((symbol, call)) = self.factory_return {
      let value = ctx.finalized_expr_for_symbol_ref(symbol, ctx.chunk_idx, &nodes.builder);
      let mut value = if call { nodes.call(value, []) } else { value };
      Generated(span).visit_expression(&mut value);
      factory_body.push(Statement::new_return_statement(span, Some(value), &nodes.builder));
    }
    if matches!(self.kind, WrapperKind::Iife)
      && self.named_exports
      && self.has_exports
      && !self.extend
    {
      factory_body.push(Statement::new_return_statement(
        span,
        Some(nodes.id("exports")),
        &nodes.builder,
      ));
    }
    if matches!(self.kind, WrapperKind::Flat) {
      return Ok(factory_body);
    }
    let factory = nodes.function(&self.parameters, factory_body);
    let mut result = ArenaVec::new_in(&nodes.builder);
    match &self.kind {
      WrapperKind::Flat => unreachable!(),
      WrapperKind::Iife => self.wrap_iife(&nodes, factory, &mut result)?,
      WrapperKind::Umd { paths } => self.wrap_umd(&nodes, factory, paths, &mut result),
    }
    Ok(result)
  }

  fn wrap_iife<'a>(
    &self,
    nodes: &Nodes<'a>,
    factory: Expression<'a>,
    result: &mut ArenaVec<'a, Statement<'a>>,
  ) -> rolldown_error::BuildResult<()> {
    let mut arguments = Vec::with_capacity(self.globals.len() + 1);
    let name = self.name.as_deref();
    let mut target = None;
    let mut declare = false;
    if let Some(name) = name {
      if name.contains('.') {
        let (initialization, namespace) =
          nodes.namespace(Expression::new_this_expression(nodes.span, &nodes.builder), name);
        result.extend(initialization.into_iter().map(|expr| nodes.expr_stmt(expr)));
        target = Some(namespace);
      } else if self.extend {
        if !name.is_empty() || self.named_exports {
          target =
            Some(nodes.member(Expression::new_this_expression(nodes.span, &nodes.builder), name));
        }
      } else {
        target = Some(nodes.id(name));
        declare = true;
      }
    }
    if self.has_exports && self.named_exports {
      if self.extend {
        if let Some(target) = &target {
          arguments.push(nodes.assign(
            target.clone_in(nodes.allocator),
            nodes.logical(target.clone_in(nodes.allocator), LogicalOperator::Or, nodes.object()),
          ));
        }
      } else {
        arguments.push(nodes.object());
      }
    }
    let mut arguments =
      ArenaVec::from_iter_in(arguments.into_iter().map(Argument::from), &nodes.builder);
    if !self.globals.is_empty() {
      // Globals are inserted as argument-list text by the renderer. Parse that user input
      // in its real grammar: commas may add arguments and spread may expand them.
      let source = nodes.allocator.alloc_str(&format!("f({});", self.globals.join(", ")));
      let parsed =
        oxc::parser::Parser::new(nodes.allocator, source, oxc::span::SourceType::mjs()).parse();
      if !parsed.diagnostics.is_empty() {
        return Err(
          anyhow::anyhow!("Invalid IIFE globals arguments: {:?}", parsed.diagnostics).into(),
        );
      }
      let mut program = parsed.program;
      let [Statement::ExpressionStatement(statement)] = program.body.as_mut_slice() else {
        return Err(anyhow::anyhow!("IIFE globals must form an argument list").into());
      };
      let Expression::CallExpression(call) = &mut statement.expression else {
        return Err(anyhow::anyhow!("IIFE globals must form an argument list").into());
      };
      for mut argument in call.arguments.take_in(&nodes.allocator) {
        Generated(nodes.span).visit_argument(&mut argument);
        arguments.push(argument);
      }
    }
    let call =
      Expression::new_call_expression(nodes.span, factory, None, arguments, false, &nodes.builder);
    if self.has_exports
      && !(self.extend && self.named_exports)
      && let Some(target) = target
    {
      if declare {
        let mut statement = Statement::new_var_decl(name.unwrap(), call, &nodes.builder);
        if let Statement::VariableDeclaration(decl) = &mut statement {
          decl.span = nodes.span;
          decl.declarations[0].span = nodes.span;
          let BindingPattern::BindingIdentifier(id) = &mut decl.declarations[0].id else {
            unreachable!("generated wrapper binding")
          };
          id.span = nodes.span;
        }
        result.push(statement);
      } else {
        result.push(nodes.expr_stmt(nodes.assign(target, call)));
      }
    } else {
      result.push(nodes.expr_stmt(call));
    }
    Ok(())
  }

  fn wrap_umd<'a>(
    &self,
    nodes: &Nodes<'a>,
    factory: Expression<'a>,
    paths: &[String],
    result: &mut ArenaVec<'a, Statement<'a>>,
  ) {
    let need_global = self.has_exports || self.named_exports || !paths.is_empty();
    let exports_argument = self.has_exports && self.named_exports;
    let mut cjs_args = Vec::new();
    let mut amd_args = Vec::new();
    let mut global_args = Vec::new();
    if exports_argument {
      cjs_args.push(nodes.id("exports"));
      amd_args.push(nodes.string("exports"));
    }
    for (path, global) in paths.iter().zip(&self.globals) {
      cjs_args.push(nodes.call(nodes.id("require"), [nodes.string(path)]));
      amd_args.push(nodes.string(path));
      let mut target = nodes.id("global");
      for part in global.split('.') {
        target = nodes.member(target, part);
      }
      global_args.push(target);
    }
    let mut namespace_initialization = Vec::new();
    let mut global_target = None;
    if self.has_exports {
      let (initialization, target) =
        nodes.namespace(nodes.id("global"), self.name.as_deref().expect("validated UMD name"));
      namespace_initialization = initialization;
      global_target = Some(target);
    }
    if exports_argument {
      let target = global_target.as_ref().unwrap();
      let value = if self.extend {
        nodes.logical(target.clone_in(nodes.allocator), LogicalOperator::Or, nodes.object())
      } else {
        nodes.object()
      };
      namespace_initialization.push(nodes.assign(target.clone_in(nodes.allocator), value));
      global_args.insert(0, nodes.sequence(std::mem::take(&mut namespace_initialization)));
    }
    let mut global_call = nodes.call(nodes.id("factory"), global_args);
    if self.has_exports && !self.named_exports {
      namespace_initialization.push(nodes.assign(global_target.unwrap(), global_call));
      global_call = nodes.sequence(namespace_initialization);
    }
    if need_global {
      let global_value = nodes.conditional(
        nodes.typeof_is("globalThis", "undefined", false),
        nodes.id("globalThis"),
        nodes.logical(nodes.id("global"), LogicalOperator::Or, nodes.id("self")),
      );
      global_call = nodes.sequence([nodes.assign(nodes.id("global"), global_value), global_call]);
    }
    let amd_test = nodes.logical(
      nodes.typeof_is("define", "function", true),
      LogicalOperator::And,
      nodes.member(nodes.id("define"), "amd"),
    );
    let dependencies = Expression::new_array_expression(
      nodes.span,
      ArenaVec::from_iter_in(
        amd_args.into_iter().map(ArrayExpressionElement::from),
        &nodes.builder,
      ),
      &nodes.builder,
    );
    let mut dispatch = nodes.conditional(
      amd_test,
      nodes.call(nodes.id("define"), [dependencies, nodes.id("factory")]),
      global_call,
    );
    if need_global {
      let cjs_test = nodes.logical(
        nodes.typeof_is("exports", "object", true),
        LogicalOperator::And,
        nodes.typeof_is("module", "undefined", false),
      );
      let mut cjs = nodes.call(nodes.id("factory"), cjs_args);
      if self.has_exports && !self.named_exports {
        cjs = nodes.assign(nodes.member(nodes.id("module"), "exports"), cjs);
      }
      dispatch = nodes.conditional(cjs_test, cjs, dispatch);
    }
    let mut body = ArenaVec::new_in(&nodes.builder);
    body.push(nodes.expr_stmt(dispatch));
    let names =
      if need_global { vec!["global".into(), "factory".into()] } else { vec!["factory".into()] };
    let wrapper = nodes.function(&names, body);
    let mut args = Vec::new();
    if need_global {
      args.push(Expression::new_this_expression(nodes.span, &nodes.builder));
    }
    args.push(factory);
    result.push(nodes.expr_stmt(nodes.call(wrapper, args)));
  }
}
