use std::borrow::Cow;

use anyhow::Ok;
use oxc::ast::ast::{
  BindingPattern, BindingPatternKind, Expression, ExpressionStatement, ImportOrExportKind,
  NumberBase, ObjectExpression, ObjectPattern, PropertyKey, Statement, StaticMemberExpression,
  TSTypeAnnotation, TSTypeParameterDeclaration, VariableDeclaration, VariableDeclarationKind,
};
use oxc::ast::visit::walk::walk_program;
use oxc::ast::visit::walk_mut;
use oxc::ast::{match_module_declaration, AstBuilder, VisitMut};
use oxc::codegen::{CodeGenerator, CodegenReturn};
use oxc::span::{Atom, SPAN};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, Plugin, PluginContext,
};
use rustc_hash::FxHashMap;

use self::utils::construct_snippet_from_import_expr;
mod utils;
#[derive(Debug)]
pub struct BuildImportAnalysisPlugin {
  pub preload_code: String,
  pub insert_preload: bool,
}

const PRELOAD_METHOD: &str = "__vitePreload";

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
    mut args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    let mut ast = args.ast;
    let CodegenReturn { source_text, source_map } = CodeGenerator::new().build(ast.program());
    println!("{:?}", args.cwd);
    println!("{}", source_text);
    ast.program.with_mut(|fields| {
      let builder = AstBuilder::new(&fields.allocator);
      let mut visitor = BuildImportAnalysisVisitor::new(&self.preload_code, builder);
      visitor.visit_program(fields.program);
    });
    let CodegenReturn { source_text, source_map } = CodeGenerator::new().build(ast.program());
    println!("after");
    println!("{:?}", args.cwd);
    println!("{}", source_text);
    Ok(ast)
  }
}

struct BuildImportAnalysisVisitor<'b, 'a: 'b> {
  preload_code: &'b str,
  builder: AstBuilder<'a>,
  need_prepend_helper: bool,
}
impl<'b, 'a> BuildImportAnalysisVisitor<'b, 'a> {
  pub fn new(preload_code: &'b str, builder: AstBuilder<'a>) -> Self {
    Self { preload_code, builder, need_prepend_helper: false }
  }
}

impl<'b, 'a> VisitMut<'a> for BuildImportAnalysisVisitor<'b, 'a> {
  fn visit_program(&mut self, it: &mut oxc::ast::ast::Program<'a>) {
    walk_mut::walk_program(self, it);
    if self.need_prepend_helper {
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
            let decl = construct_snippet_from_import_expr(&self.builder, source, decls, kind);
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

  // fn visit_expression_statement(&mut self, it: &mut ExpressionStatement<'a>) {}
}

/// Consider the case:
/// ```js
/// const {a} = await import('./lib.js'), {b} = await import('./lib.js');
/// console.log(`a: `, a, b)
/// ```
/// return value should map to each declarator
// fn extract_from_var_decl<'a>(
//   declaration: &'a mut VariableDeclaration<'a>,
//   builder: &AstBuilder<'a>,
// ) -> FxHashMap<usize, (ImportPattern<'a>, VariableDeclarationKind)> {
// }

fn extract_from_expr_stmt<'a>(stmt: &StaticMemberExpression<'a>) -> Option<ImportPattern<'a>> {
  todo!()
}
