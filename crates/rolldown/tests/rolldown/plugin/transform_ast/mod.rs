use std::borrow::Cow;

use oxc::{
  allocator::CloneIn,
  ast::{
    ast::{
      Declaration, Expression, ImportOrExportKind, ModuleDeclaration, ParenthesizedExpression,
      Statement, StringLiteral,
    },
    builder::NONE,
  },
  span::{GetSpan, SPAN, SourceType},
};
use rolldown_common::ModuleType;
use rolldown_ecmascript::EcmaCompiler;
use rolldown_ecmascript_utils::AstFactory;
use rolldown_plugin::{
  HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin, PluginContext,
};

#[derive(Debug)]
enum JsonMutation {
  AppendNamedExport,
  GetterProperty,
  PrependExpression,
  PrependStaticImport,
  MovePayloadAfterExpression,
  ClonePayloadAndAppendParenthesizedExpression,
}

#[derive(Debug)]
struct JsonTransformAstPlugin {
  mutation: JsonMutation,
}

impl JsonTransformAstPlugin {
  fn new(mutation: JsonMutation) -> Self {
    Self { mutation }
  }
}

impl Plugin for JsonTransformAstPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("json-transform-ast")
  }

  async fn transform_ast(
    &self,
    _ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    if !matches!(args.module_type, ModuleType::Json) {
      return Ok(args.ast);
    }

    let appended_expression_ast = if matches!(
      self.mutation,
      JsonMutation::MovePayloadAfterExpression
        | JsonMutation::ClonePayloadAndAppendParenthesizedExpression
    ) {
      Some(EcmaCompiler::parse_expr_as_program(
        "appended-expression.js",
        "globalThis.jsonAppendedExpressionRan = true",
        SourceType::default().with_module(true),
      )?)
    } else {
      None
    };
    let getter_ast = if matches!(self.mutation, JsonMutation::GetterProperty) {
      Some(EcmaCompiler::parse_expr_as_program(
        "getter-property.js",
        "({ get normal() { globalThis.jsonGetterReads = (globalThis.jsonGetterReads ?? 0) + 1; return 4; }, stable: 1 })",
        SourceType::default().with_module(true),
      )?)
    } else {
      None
    };

    args.ast.program.with_mut(|fields| {
      let ast_factory = AstFactory::new(fields.allocator);
      if matches!(
        self.mutation,
        JsonMutation::MovePayloadAfterExpression
          | JsonMutation::ClonePayloadAndAppendParenthesizedExpression
      ) {
        let Some(Statement::ExpressionStatement(statement)) = fields.program.body.first() else {
          return;
        };
        assert!(
          matches!(&statement.expression, Expression::ObjectExpression(_)),
          "transformAst must observe the loader-created JSON payload without an internal wrapper"
        );
        let span = statement.span();
        assert!(
          fields.source.get(span.start as usize..span.end as usize).is_some(),
          "the loader-created payload span must stay inside the transformAst source"
        );
      }
      match self.mutation {
        JsonMutation::MovePayloadAfterExpression => {
          let payload = fields.program.body.remove(0);
          let Some(appended_expression_ast) = appended_expression_ast.as_ref() else { return };
          let Some(Statement::ExpressionStatement(source)) =
            appended_expression_ast.program().body.first()
          else {
            return;
          };
          fields.program.body.push(Statement::new_expression_statement(
            SPAN,
            source.expression.clone_in(fields.allocator),
            &ast_factory,
          ));
          fields.program.body.push(payload);
        }
        JsonMutation::ClonePayloadAndAppendParenthesizedExpression => {
          let Some(replacement) =
            fields.program.body.first().map(|statement| statement.clone_in(fields.allocator))
          else {
            return;
          };
          fields.program.body[0] = replacement;
          let Some(appended_expression_ast) = appended_expression_ast.as_ref() else { return };
          let Some(Statement::ExpressionStatement(source)) =
            appended_expression_ast.program().body.first()
          else {
            return;
          };
          let mut expression = source.expression.clone_in(fields.allocator);
          for _ in 0..4 {
            expression = Expression::ParenthesizedExpression(ParenthesizedExpression::boxed(
              SPAN,
              expression,
              &ast_factory,
            ));
          }
          fields.program.body.push(Statement::new_expression_statement(
            SPAN,
            expression,
            &ast_factory,
          ));
        }
        JsonMutation::AppendNamedExport => {
          let declaration = ast_factory.make_var_decl(
            "injected",
            ast_factory.make_call_with_arg(
              ast_factory.make_id_ref_expr(SPAN, "Number"),
              Expression::new_string_literal(SPAN, "2", None, &ast_factory),
              false,
            ),
          );
          let Statement::VariableDeclaration(declaration) = declaration else { unreachable!() };
          fields.program.body.push(Statement::from(
            ModuleDeclaration::new_export_named_declaration(
              SPAN,
              Some(Declaration::VariableDeclaration(declaration)),
              oxc::allocator::Vec::new_in(&ast_factory),
              None,
              ImportOrExportKind::Value,
              NONE,
              &ast_factory,
            ),
          ));
        }
        JsonMutation::GetterProperty => {
          let Some(Statement::ExpressionStatement(target)) = fields.program.body.first_mut() else {
            return;
          };
          let Some(getter_ast) = getter_ast.as_ref() else { return };
          let Some(Statement::ExpressionStatement(source)) = getter_ast.program().body.first()
          else {
            return;
          };
          target.expression = source.expression.clone_in(fields.allocator);
        }
        JsonMutation::PrependExpression => {
          let expression = ast_factory.make_call_with_arg(
            ast_factory.make_id_ref_expr(SPAN, "Number"),
            Expression::new_string_literal(SPAN, "3", None, &ast_factory),
            false,
          );
          fields
            .program
            .body
            .insert(0, Statement::new_expression_statement(SPAN, expression, &ast_factory));
        }
        JsonMutation::PrependStaticImport => {
          fields.program.body.insert(
            0,
            Statement::from(ModuleDeclaration::new_import_declaration(
              SPAN,
              None,
              StringLiteral::new(SPAN, "./side-effect.js", None, &ast_factory),
              None,
              NONE,
              ImportOrExportKind::Value,
              &ast_factory,
            )),
          );
        }
      }
    });

    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst
  }
}

mod json_ambiguous_replaced_payload;
mod json_appended_expression;
mod json_cycle_snapshot;
mod json_entry_invalid_key;
mod json_getter_property;
mod json_iife_umd_invalid_key;
mod json_invalid_key_common_chunk_cjs;
mod json_named_export;
mod json_namespace_import;
mod json_replaced_payload_entry;
mod json_static_import;
mod json_static_import_invalid_key;
mod json_static_import_mutation;
mod json_static_import_prepended_expression;
mod json_static_import_preserve_modules;
mod json_static_import_proto_key;
mod json_static_import_tree_shake;
