use std::borrow::Cow;

use anyhow::Ok;
use oxc::ast::ast::{
  Argument, BindingPattern, BindingPatternKind, CallExpression, Expression, ExpressionStatement,
  ImportOrExportKind, PropertyKey, StaticMemberExpression, TSTypeAnnotation, VariableDeclaration,
  VariableDeclarationKind,
};
use oxc::ast::visit::walk::walk_ts_call_signature_declaration;
use oxc::ast::visit::walk_mut;
use oxc::ast::{ast_builder, AstBuilder, VisitMut};
use oxc::span::{Atom, SPAN};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, Plugin, PluginContext,
};
use rustc_hash::FxHashMap;

use self::utils::{construct_snippet_from_await_decl, construct_snippet_from_import_then};
mod utils;
#[derive(Debug)]
pub struct BuildImportAnalysisPlugin {
  pub preload_code: String,
  pub insert_preload: bool,
  pub optimize_module_preload_relative_paths: bool,
}

const PRELOAD_METHOD: &str = "__vitePreload";

pub const IS_MODERN_FLAG: &str = "__VITE_IS_MODERN__";

// TODO:replace `\t` with `\0`
const PRELOAD_HELPER_ID: &str = "\tvite/preload-helper.js";

/// First element is the import specifier, second element is `decls` or `props` of expr
enum ImportPattern<'a> {
  /// (await import('foo')).foo
  MemberExpr(Atom<'a>, Vec<Atom<'a>>),
  /// const {foo} = await import('foo')
  Decl(Atom<'a>, Vec<Atom<'a>>),
  /// import('foo').then(({foo})=>{})
  ImportExpr(Atom<'a>, Vec<Atom<'a>>),
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
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.to_string(),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id == PRELOAD_HELPER_ID {
      return Ok(Some(HookLoadOutput { code: self.preload_code.clone(), ..Default::default() }));
    }
    Ok(None)
  }

  fn transform_ast(
    &self,
    _ctx: &PluginContext,
    args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    if args.id.contains("node_modules") {
      return Ok(args.ast);
    }
    let mut ast = args.ast;
    ast.program.with_mut(|fields| {
      let builder = AstBuilder::new(&fields.allocator);
      let mut visitor = BuildImportAnalysisVisitor::new(builder, self.insert_preload);
      visitor.visit_program(fields.program);
    });
    Ok(ast)
  }
}

struct BuildImportAnalysisVisitor<'a> {
  builder: AstBuilder<'a>,
  need_prepend_helper: bool,
  insert_preload: bool,
}
impl<'a> BuildImportAnalysisVisitor<'a> {
  pub fn new(builder: AstBuilder<'a>, insert_preload: bool) -> Self {
    Self { builder, need_prepend_helper: false, insert_preload }
  }
}

impl<'a> VisitMut<'a> for BuildImportAnalysisVisitor<'a> {
  fn visit_program(&mut self, it: &mut oxc::ast::ast::Program<'a>) {
    walk_mut::walk_program(self, it);
    // TODO: passing scope to detect if the helper is inserted before
    if self.need_prepend_helper && self.insert_preload {
      let helper_stmt = self.builder.statement_module_declaration(
        self.builder.module_declaration_import_declaration(
          SPAN,
          Some(self.builder.vec1(self.builder.import_declaration_specifier_import_specifier(
            SPAN,
            self.builder.module_export_name_identifier_name(SPAN, PRELOAD_METHOD),
            self.builder.binding_identifier(SPAN, PRELOAD_METHOD),
            ImportOrExportKind::Value,
          ))),
          self.builder.string_literal(SPAN, PRELOAD_HELPER_ID),
          None,
          ImportOrExportKind::Value,
        ),
      );
      it.body.push(helper_stmt);
    }
  }

  fn visit_variable_declaration(&mut self, decl: &mut VariableDeclaration<'a>) {
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
          Expression::StringLiteral(lit) => lit.value.clone(),
          Expression::TemplateLiteral(lit)
            if lit.quasis.len() == 1 && lit.expressions.is_empty() =>
          {
            let first = lit.quasis.first()?;
            first.value.cooked.clone().unwrap_or(first.value.raw.clone())
          }
          _ => return None,
        };
        // TODO: `const {a: {c: {d: f}}} = await import('./lib.js')`
        // for now, only support `const {a} = await import('./lib.js')`
        let decls = kind
          .properties
          .iter()
          .filter_map(|prop| match &prop.key {
            PropertyKey::StaticIdentifier(id) => Some(id.name.clone()),
            _ => None,
          })
          .collect::<Vec<_>>();
        Some((i, (ImportPattern::Decl(source, decls), decl.kind)))
      })
      .collect::<FxHashMap<usize, (ImportPattern<'a>, VariableDeclarationKind)>>();

    let mut ret = vec![];
    for (i, d) in decl.declarations.iter_mut().enumerate() {
      if let Some((pattern, kind)) = declarators_map.remove(&i) {
        match pattern {
          ImportPattern::Decl(source, decls) => {
            self.need_prepend_helper = true;
            let decl = construct_snippet_from_await_decl(&self.builder, source, decls, kind);
            ret.push(decl);
          }
          _ => {
            unreachable!()
          }
        }
      } else {
        let dummy = self.builder.variable_declarator(
          SPAN,
          VariableDeclarationKind::Var,
          self.builder.binding_pattern(
            self.builder.binding_pattern_kind_binding_identifier(SPAN, "a"),
            None::<TSTypeAnnotation>,
            false,
          ),
          Some(self.builder.expression_null_literal(SPAN)),
          false,
        );
        ret.push(std::mem::replace(d, dummy))
      }
    }
    decl.declarations = self.builder.vec_from_iter(ret);
  }

  fn visit_expression_statement(&mut self, it: &mut ExpressionStatement<'a>) {
    let Some(pat) = extract_from_expr_stmt(it) else {
      return;
    };

    match pat {
      ImportPattern::MemberExpr(_, _) => unreachable!(),
      ImportPattern::Decl(_, _) => unreachable!(),
      ImportPattern::ImportExpr(source, decls) => {
        it.expression = construct_snippet_from_import_then(&self.builder, source, decls);
        self.need_prepend_helper = true;
      }
    }
  }
}

fn extract_from_expr_stmt<'a>(stmt: &ExpressionStatement<'a>) -> Option<ImportPattern<'a>> {
  let expr = &stmt.expression;
  match expr {
    Expression::StaticMemberExpression(expr) => extract_from_static_member_expr(expr),
    Expression::CallExpression(expr) => extract_from_call_expr(expr),
    _ => return None,
  }
}

/// ```js
/// import('foo').then(({foo})=>{})
/// ```
fn extract_from_call_expr<'a>(expr: &CallExpression<'a>) -> Option<ImportPattern<'a>> {
  let Expression::StaticMemberExpression(ref callee) = expr.callee else {
    return None;
  };
  let Expression::ImportExpression(ref import_expr) = callee.object else {
    return None;
  };
  let source = match &import_expr.source {
    Expression::StringLiteral(lit) => lit.value.clone(),
    Expression::TemplateLiteral(lit) if lit.quasis.len() == 1 && lit.expressions.is_empty() => {
      let first = lit.quasis.first()?;
      first.value.cooked.clone().unwrap_or(first.value.raw.clone())
    }
    _ => return None,
  };
  if callee.property.name != "then" {
    return None;
  };
  let arrow_expr = match expr.arguments.as_slice() {
    [Argument::ArrowFunctionExpression(arrow)] => arrow,
    _ => return None,
  };
  let first_param = arrow_expr.params.items.first()?;

  let BindingPatternKind::ObjectPattern(ref pat) = first_param.pattern.kind else {
    return None;
  };

  let decls = pat
    .properties
    .iter()
    .filter_map(|prop| match &prop.key {
      PropertyKey::StaticIdentifier(id) => Some(id.name.clone()),
      _ => None,
    })
    .collect::<Vec<_>>();
  Some(ImportPattern::ImportExpr(source, decls))
}

/// ```js
/// const {foo} = await import('foo')
/// ```
fn extract_from_static_member_expr<'a>(
  member_expr: &StaticMemberExpression<'a>,
) -> Option<ImportPattern<'a>> {
  let Expression::ParenthesizedExpression(ref paren) = member_expr.object else {
    return None;
  };
  let Expression::AwaitExpression(ref expr) = paren.expression else {
    return None;
  };
  let Expression::ImportExpression(ref import_expr) = expr.argument else { return None };

  let source = match &import_expr.source {
    Expression::StringLiteral(lit) => lit.value.clone(),
    Expression::TemplateLiteral(lit) if lit.quasis.len() == 1 && lit.expressions.is_empty() => {
      let first = lit.quasis.first()?;
      first.value.cooked.clone().unwrap_or(first.value.raw.clone())
    }
    _ => return None,
  };

  let property = member_expr.property.name.clone();
  Some(ImportPattern::ImportExpr(source, vec![property]))
}
