use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils;
use oxc::{
  ast::{
    AstBuilder, NONE,
    ast::{
      Argument, Expression, ImportOrExportKind, PropertyKind, Statement, TemplateElementValue,
    },
  },
  ast_visit::{VisitMut, walk_mut},
  span::SPAN,
  syntax::number::NumberBase,
};
use rolldown_plugin::{LogWithoutPlugin, PluginContext};
use sugar_path::SugarPath as _;

use super::DYNAMIC_IMPORT_HELPER;
use super::dynamic_import_to_glob::{
  has_special_query_param, should_ignore, template_literal_to_glob, to_valid_glob,
};

#[derive(Debug)]
struct DynamicImportRequest<'a> {
  pub query: &'a str,
  pub import: bool,
}

pub struct DynamicImportVarsVisit<'ast, 'b> {
  pub ctx: &'b PluginContext,
  pub source_text: &'ast str,
  pub ast_builder: AstBuilder<'ast>,
  pub root: &'b Path,
  pub importer: &'b Path,
  pub need_helper: bool,
  pub async_imports: Vec<String>,
  pub async_imports_addrs: Vec<*mut Expression<'ast>>,
}

impl<'ast> VisitMut<'ast> for DynamicImportVarsVisit<'ast, '_> {
  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    if self.rewrite_variable_dynamic_import(expr, None) {
      walk_mut::walk_expression(self, expr);
    }
  }
}

impl<'ast> DynamicImportVarsVisit<'ast, '_> {
  pub fn rewrite_variable_dynamic_import(
    &mut self,
    expr: &mut Expression<'ast>,
    async_imports: Option<&str>,
  ) -> bool {
    if let Expression::ImportExpression(import_expr) = expr
      && let Expression::TemplateLiteral(source) = &mut import_expr.source
    {
      let glob = match async_imports {
        Some(glob) => Cow::Borrowed(glob),
        None => {
          if source.is_no_substitution_template() {
            return false;
          }

          let glob = match template_literal_to_glob(source) {
            Ok(glob) => glob,
            Err(error) => {
              self.ctx.warn(LogWithoutPlugin { message: error.to_string(), ..Default::default() });
              return false;
            }
          };

          if memchr::memchr(b'*', glob.as_bytes()).is_none() || should_ignore(&glob) {
            return false;
          }

          if glob.as_bytes()[0] != b'.' && glob.as_bytes()[0] != b'/' {
            self.async_imports.push(glob.into_owned());
            self.async_imports_addrs.push(std::ptr::from_mut(expr));
            return false;
          }

          glob
        }
      };

      let base = self.importer.parent().unwrap_or(self.root);
      let normalized = if glob.as_bytes()[0] == b'/' {
        self.root.join(&glob[1..]).relative(base)
      } else {
        base.join(glob.as_ref()).relative(base)
      };

      let glob = normalized.to_slash_lossy();
      let glob = if glob.as_bytes()[0] == b'.' {
        glob.into_owned()
      } else {
        rolldown_utils::concat_string!("./", glob)
      };

      let Some(index) = memchr::memchr(b'*', glob.as_bytes()) else {
        return false;
      };

      let mut raw_value = None;
      if &glob[..index] != source.quasis[0].value.raw {
        raw_value = Some(TemplateElementValue {
          raw: source.quasis[0].value.raw,
          cooked: source.quasis[0].value.cooked.take(),
        });
        source.quasis[0].value.raw = self.ast_builder.atom(&glob[..index]);
      }

      let glob = glob.cow_replace("**", "*");
      let source_text = source.span.source_text(self.source_text);

      let (pattern, glob_params) = {
        let index = glob.rfind('/').unwrap_or(0);
        let index = glob[index..].find('?').map_or(glob.len(), |i| i + index);

        let (glob, query) = glob.split_at(index);
        let glob = match to_valid_glob(glob, source_text) {
          Ok(glob) => glob,
          Err(error) => {
            self.ctx.warn(LogWithoutPlugin { message: error.to_string(), ..Default::default() });
            if let Some(raw_value) = raw_value {
              source.quasis[0].value = raw_value;
            }
            return false;
          }
        };

        let params = (!query.is_empty())
          .then_some(DynamicImportRequest { query, import: has_special_query_param(query) });

        (glob, params)
      };

      *expr = self.variable_dynamic_import_runtime_helper_call(
        &pattern,
        std::mem::replace(&mut import_expr.source, self.ast_builder.expression_null_literal(SPAN)),
        glob_params,
      );

      self.need_helper = true;
      return false;
    }
    true
  }

  /// ```js
  /// __variableDynamicImportRuntimeHelper((import.meta.glob(pattern, params)), expr, segments)
  /// ```
  #[expect(clippy::cast_precision_loss)]
  fn variable_dynamic_import_runtime_helper_call(
    &self,
    pattern: &str,
    raw_expr: Expression<'ast>,
    glob_params: Option<DynamicImportRequest>,
  ) -> Expression<'ast> {
    let segments = pattern.split('/').count();
    self.ast_builder.expression_call(
      SPAN,
      self.ast_builder.expression_identifier(SPAN, "__variableDynamicImportRuntimeHelper"),
      NONE,
      {
        let mut items = self.ast_builder.vec_with_capacity(3);
        items.push(Argument::from(self.ast_builder.expression_call(
          SPAN,
          Expression::from(self.ast_builder.member_expression_static(
            SPAN,
            self.ast_builder.expression_meta_property(
              SPAN,
              self.ast_builder.identifier_name(SPAN, "import"),
              self.ast_builder.identifier_name(SPAN, "meta"),
            ),
            self.ast_builder.identifier_name(SPAN, "glob"),
            false,
          )),
          NONE,
          {
            let mut arguments =
              self.ast_builder.vec_with_capacity(if glob_params.is_some() { 2 } else { 1 });
            arguments.push(Argument::from(self.ast_builder.expression_string_literal(
              SPAN,
              self.ast_builder.atom(pattern),
              None,
            )));

            if let Some(params) = glob_params {
              arguments.push(Argument::from(self.ast_builder.expression_object(SPAN, {
                let mut items =
                  self.ast_builder.vec_with_capacity(if params.import { 2 } else { 1 });
                items.push(self.ast_builder.object_property_kind_object_property(
                  SPAN,
                  PropertyKind::Init,
                  self.ast_builder.property_key_static_identifier(SPAN, "query"),
                  self.ast_builder.expression_string_literal(
                    SPAN,
                    self.ast_builder.atom(params.query),
                    None,
                  ),
                  false,
                  false,
                  false,
                ));
                if params.import {
                  items.push(self.ast_builder.object_property_kind_object_property(
                    SPAN,
                    PropertyKind::Init,
                    self.ast_builder.property_key_static_identifier(SPAN, "import"),
                    self.ast_builder.expression_string_literal(SPAN, "*", None),
                    false,
                    false,
                    false,
                  ));
                }
                items
              })));
            }
            arguments
          },
          false,
        )));
        items.push(Argument::from(raw_expr));
        items.push(Argument::from(self.ast_builder.expression_numeric_literal(
          SPAN,
          segments as f64,
          None,
          NumberBase::Decimal,
        )));
        items
      },
      false,
    )
  }

  /// ```js
  /// import __variableDynamicImportRuntimeHelper from "${dynamicImportHelperId}";
  /// ```
  pub fn variable_dynamic_import_runtime_helper(&self) -> Statement<'ast> {
    Statement::from(self.ast_builder.module_declaration_import_declaration(
      SPAN,
      Some(self.ast_builder.vec1(
        self.ast_builder.import_declaration_specifier_import_default_specifier(
          SPAN,
          self.ast_builder.binding_identifier(SPAN, "__variableDynamicImportRuntimeHelper"),
        ),
      )),
      self.ast_builder.string_literal(SPAN, DYNAMIC_IMPORT_HELPER, None),
      None,
      NONE,
      ImportOrExportKind::Value,
    ))
  }
}
