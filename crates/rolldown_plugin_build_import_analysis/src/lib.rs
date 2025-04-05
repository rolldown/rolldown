use std::borrow::Cow;

use anyhow::Ok;
use oxc::ast::ast::{
  Argument, BindingPattern, BindingPatternKind, CallExpression, Expression, ImportOrExportKind,
  PropertyKey, Statement, StaticMemberExpression, VariableDeclaration, VariableDeclarationKind,
};
use oxc::ast::{AstBuilder, NONE};
use oxc::ast_visit::{VisitMut, walk_mut};
use oxc::codegen::{self, CodeGenerator, Gen};
use oxc::semantic::ScopeFlags;
use oxc::span::{Atom, SPAN};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, Plugin, PluginContext,
};
use rustc_hash::FxHashMap;

use self::utils::{construct_snippet_for_expression, construct_snippet_from_await_decl};
mod utils;

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BuildImportAnalysisPlugin {
  pub preload_code: String,
  pub insert_preload: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
}

const PRELOAD_METHOD: &str = "__vitePreload";

pub const IS_MODERN_FLAG: &str = "__VITE_IS_MODERN__";

const PRELOAD_HELPER_ID: &str = "\0vite/preload-helper.js";

/// First element is the import specifier, second element is `decls` or `props` of expr
#[derive(Debug)]
enum ImportPattern<'a> {
  /// const {foo} = await import('foo')
  Decl(Atom<'a>, Vec<Atom<'a>>),
}
impl Plugin for BuildImportAnalysisPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:build-import-analysis")
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == PRELOAD_HELPER_ID {
      return Ok(Some(HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }));
    }
    Ok(None)
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id == PRELOAD_HELPER_ID {
      return Ok(Some(HookLoadOutput { code: self.preload_code.clone(), ..Default::default() }));
    }
    Ok(None)
  }

  async fn transform_ast(
    &self,
    _ctx: &PluginContext,
    args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    if args.stable_id.contains("node_modules") {
      return Ok(args.ast);
    }
    let mut ast = args.ast;
    ast.program.with_mut(|fields| {
      let builder = AstBuilder::new(fields.allocator);
      let mut visitor = BuildImportAnalysisVisitor::new(
        builder,
        self.insert_preload,
        self.render_built_url,
        self.is_relative_base,
      );
      visitor.visit_program(fields.program);
    });

    let mut codegen = CodeGenerator::new();
    ast.program().r#gen(&mut codegen, codegen::Context::default());
    Ok(ast)
  }
}

#[allow(clippy::struct_excessive_bools)]
struct BuildImportAnalysisVisitor<'a> {
  builder: AstBuilder<'a>,
  need_prepend_helper: bool,
  insert_preload: bool,
  scope_stack: Vec<ScopeFlags>,
  has_inserted_helper: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
}
impl<'a> BuildImportAnalysisVisitor<'a> {
  pub fn new(
    builder: AstBuilder<'a>,
    insert_preload: bool,
    render_built_url: bool,
    is_relative_base: bool,
  ) -> Self {
    Self {
      builder,
      need_prepend_helper: false,
      insert_preload,
      scope_stack: vec![],
      has_inserted_helper: false,
      render_built_url,
      is_relative_base,
    }
  }

  fn is_top_level(&self) -> bool {
    self.scope_stack.last().is_some_and(|flags| flags.contains(ScopeFlags::Top))
  }

  /// ```js
  /// (await import('foo')).foo
  /// ```
  fn rewrite_paren_member_expr(&mut self, member_expr: &mut StaticMemberExpression<'a>) {
    let Expression::ParenthesizedExpression(ref mut paren) = member_expr.object else {
      return;
    };
    let Expression::AwaitExpression(ref mut expr) = paren.expression else {
      return;
    };
    let Expression::ImportExpression(ref import_expr) = expr.argument else { return };

    let source = match &import_expr.source {
      Expression::StringLiteral(lit) => lit.value,
      Expression::TemplateLiteral(lit) if lit.quasis.len() == 1 && lit.expressions.is_empty() => {
        let Some(first) = lit.quasis.first() else {
          return;
        };
        first.value.cooked.unwrap_or(first.value.raw)
      }
      _ => return,
    };

    let property = member_expr.property.name;

    let vite_preload_call = construct_snippet_for_expression(
      self.builder,
      source,
      &[property],
      self.is_relative_base || self.render_built_url,
    );
    expr.argument = vite_preload_call;
    self.need_prepend_helper = true;
  }

  /// ```js
  /// import('foo').then(({foo})=>{})
  /// ```
  fn rewrite_import_expr(&mut self, expr: &mut CallExpression<'a>) {
    let Expression::StaticMemberExpression(ref mut callee) = expr.callee else {
      return;
    };
    let Expression::ImportExpression(ref import_expr) = callee.object else {
      return;
    };
    let source = match &import_expr.source {
      Expression::StringLiteral(lit) => lit.value,
      Expression::TemplateLiteral(lit) if lit.quasis.len() == 1 && lit.expressions.is_empty() => {
        let Some(first) = lit.quasis.first() else { return };
        first.value.cooked.unwrap_or(first.value.raw)
      }
      _ => return,
    };
    if callee.property.name != "then" {
      return;
    }
    let [Argument::ArrowFunctionExpression(arrow_expr)] = expr.arguments.as_slice() else {
      return;
    };
    let Some(first_param) = arrow_expr.params.items.first() else {
      return;
    };

    let BindingPatternKind::ObjectPattern(ref pat) = first_param.pattern.kind else {
      return;
    };

    let decls = pat
      .properties
      .iter()
      .filter_map(|prop| match &prop.key {
        PropertyKey::StaticIdentifier(id) => Some(id.name),
        _ => None,
      })
      .collect::<Vec<_>>();

    let vite_preload_call = construct_snippet_for_expression(
      self.builder,
      source,
      &decls,
      self.render_built_url || self.is_relative_base,
    );
    callee.object = vite_preload_call;
    self.need_prepend_helper = true;
  }
}

impl<'a> VisitMut<'a> for BuildImportAnalysisVisitor<'a> {
  fn visit_program(&mut self, it: &mut oxc::ast::ast::Program<'a>) {
    walk_mut::walk_program(self, it);
    if self.need_prepend_helper && self.insert_preload && !self.has_inserted_helper {
      let helper_stmt = Statement::from(self.builder.module_declaration_import_declaration(
        SPAN,
        Some(self.builder.vec1(self.builder.import_declaration_specifier_import_specifier(
          SPAN,
          self.builder.module_export_name_identifier_name(SPAN, PRELOAD_METHOD),
          self.builder.binding_identifier(SPAN, PRELOAD_METHOD),
          ImportOrExportKind::Value,
        ))),
        self.builder.string_literal(SPAN, PRELOAD_HELPER_ID, None),
        None,
        NONE,
        ImportOrExportKind::Value,
      ));
      it.body.push(helper_stmt);
    }
  }

  fn visit_variable_declaration(&mut self, decl: &mut VariableDeclaration<'a>) {
    walk_mut::walk_variable_declaration(self, decl);
    let mut declarators_map = decl
      .declarations
      .iter_mut()
      .enumerate()
      .filter_map(|(i, decl)| {
        let Some(Expression::AwaitExpression(ref mut init)) = decl.init else {
          return None;
        };
        let Expression::ImportExpression(ref mut import) = init.argument else {
          return None;
        };
        let BindingPattern { kind, .. } = &decl.id;
        let BindingPatternKind::ObjectPattern(kind) = kind else {
          return None;
        };
        let source = match &import.source {
          Expression::StringLiteral(lit) => lit.value,
          Expression::TemplateLiteral(lit)
            if lit.quasis.len() == 1 && lit.expressions.is_empty() =>
          {
            let first = lit.quasis.first()?;
            first.value.cooked.unwrap_or(first.value.raw)
          }
          _ => return None,
        };
        // TODO: `const {a: {c: {d: f}}} = await import('./lib.js')`
        // for now, only support `const {a} = await import('./lib.js')`
        let decls = kind
          .properties
          .iter()
          .filter_map(|prop| match &prop.key {
            PropertyKey::StaticIdentifier(id) => Some(id.name),
            _ => None,
          })
          .collect::<Vec<_>>();
        Some((i, (ImportPattern::Decl(source, decls), decl.kind)))
      })
      .collect::<FxHashMap<usize, (ImportPattern<'a>, VariableDeclarationKind)>>();
    if declarators_map.is_empty() {
      return;
    }
    for (i, d) in decl.declarations.iter_mut().enumerate() {
      if let Some((pattern, kind)) = declarators_map.remove(&i) {
        match pattern {
          ImportPattern::Decl(source, decls) => {
            self.need_prepend_helper = true;
            let mut declarator = construct_snippet_from_await_decl(
              self.builder,
              source,
              &decls,
              kind,
              self.render_built_url || self.is_relative_base,
            );
            std::mem::swap(d, &mut declarator);
          }
        }
      }
    }
  }

  fn visit_variable_declarator(&mut self, it: &mut oxc::ast::ast::VariableDeclarator<'a>) {
    // Only check if there needs to insert helper function
    if self.insert_preload && self.is_top_level() {
      if let BindingPatternKind::BindingIdentifier(id) = &it.id.kind {
        self.has_inserted_helper = id.name == PRELOAD_METHOD;
      }
    }
    walk_mut::walk_variable_declarator(self, it);
  }

  fn visit_expression(&mut self, expr: &mut Expression<'a>) {
    walk_mut::walk_expression(self, expr);
    match expr {
      Expression::StaticMemberExpression(expr) => self.rewrite_paren_member_expr(expr),
      Expression::CallExpression(expr) => self.rewrite_import_expr(expr),
      _ => {}
    }
  }

  fn enter_scope(
    &mut self,
    flags: ScopeFlags,
    _scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(flags);
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
  }
}
