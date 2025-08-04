use oxc::{
  ast::{NONE, ast},
  span::SPAN,
};
use rolldown_common::NormalModule;

use crate::{hmr::hmr_ast_finalizer::HmrAstFinalizer, module_finalizers::ScopeHoistingFinalizer};

pub static MODULE_EXPORTS_NAME_FOR_ESM: &str = "__rolldown_exports__";

pub trait HmrAstBuilder<'any, 'ast> {
  fn builder(&self) -> &oxc::ast::AstBuilder<'ast>;

  fn module(&self) -> &NormalModule;

  // `${ns_name}` in `var ${ns_name} = ...`
  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Atom<'ast>;

  // `$hot_name` in var `$hot_name = __rolldown_runtime__.createModuleHotContext($stable_id);`
  fn alias_name_for_import_meta_hot(&self) -> ast::Atom<'ast>;

  /// `__rolldown_runtime__.registerModule(moduleId, module)`
  fn create_register_module_stmt(&self) -> ast::Statement<'ast> {
    let module_exports = match self.module().exports_kind {
      rolldown_common::ExportsKind::Esm => {
        let binding_name_for_namespace_object_ref_atom =
          self.binding_name_for_namespace_object_ref_atom();
        let namespace_object_ref_expr = ast::Expression::Identifier(
          self
            .builder()
            .alloc_identifier_reference(SPAN, binding_name_for_namespace_object_ref_atom),
        );

        // { exports: namespace }
        ast::Argument::ObjectExpression(self.builder().alloc_object_expression(
          SPAN,
          self.builder().vec1(self.builder().object_property_kind_object_property(
            SPAN,
            ast::PropertyKind::Init,
            self.builder().property_key_static_identifier(SPAN, "exports"),
            namespace_object_ref_expr,
            true,
            false,
            false,
          )),
        ))
      }
      rolldown_common::ExportsKind::CommonJs => {
        // `module`
        ast::Argument::from(ast::Expression::Identifier(
          self.builder().alloc_identifier_reference(SPAN, "module"),
        ))
      }
      rolldown_common::ExportsKind::None => {
        // `{}`
        ast::Argument::from(ast::Expression::ObjectExpression(
          self.builder().alloc_object_expression(SPAN, self.builder().vec()),
        ))
      }
    };

    // ...(moduleId, module)
    let arguments = self.builder().vec_from_array([
      ast::Argument::StringLiteral(self.builder().alloc_string_literal(
        SPAN,
        self.builder().atom(&self.module().stable_id),
        None,
      )),
      module_exports,
    ]);

    // __rolldown_runtime__.registerModule(moduleId, module)
    let register_call = self.builder().alloc_call_expression(
      SPAN,
      ast::Expression::Identifier(
        self.builder().alloc_identifier_reference(SPAN, "__rolldown_runtime__.registerModule"),
      ),
      NONE,
      arguments,
      false,
    );

    ast::Statement::ExpressionStatement(
      self
        .builder()
        .alloc_expression_statement(SPAN, ast::Expression::CallExpression(register_call)),
    )
  }

  /// `var $hot_name = __rolldown_runtime__.createModuleHotContext($stable_id);`
  fn create_module_hot_context_initializer_stmt(&self) -> ast::Statement<'ast> {
    // var $hot_name = __rolldown_runtime__.createModuleHotContext($stable_id);
    ast::Statement::VariableDeclaration(
      self.builder().alloc_variable_declaration(
        SPAN,
        ast::VariableDeclarationKind::Const,
        self.builder().vec1(
          // var $hot_name
          self.builder().variable_declarator(
            SPAN,
            ast::VariableDeclarationKind::Const,
            self.builder().binding_pattern(
              self.builder().binding_pattern_kind_binding_identifier(
                SPAN,
                self.alias_name_for_import_meta_hot(),
              ),
              NONE,
              false,
            ),
            // __rolldown_runtime__.createModuleHotContext($stable_id)
            Some(ast::Expression::CallExpression(
              self.builder().alloc_call_expression(
                SPAN,
                ast::Expression::Identifier(
                  self.builder().alloc_identifier_reference(
                    SPAN,
                    "__rolldown_runtime__.createModuleHotContext",
                  ),
                ),
                NONE,
                self.builder().vec1(ast::Argument::StringLiteral(
                  self.builder().alloc_string_literal(
                    SPAN,
                    self.builder().atom(&self.module().stable_id),
                    None,
                  ),
                )),
                false,
              ),
            )),
            false,
          ),
        ),
        false,
      ),
    )
  }
}

impl<'any, 'ast> HmrAstBuilder<'any, 'ast> for HmrAstFinalizer<'any, 'ast> {
  fn builder(&self) -> &oxc::ast::AstBuilder<'ast> {
    self.builder
  }

  fn module(&self) -> &NormalModule {
    self.module
  }

  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Atom<'ast> {
    self.builder().atom(MODULE_EXPORTS_NAME_FOR_ESM)
  }

  fn alias_name_for_import_meta_hot(&self) -> ast::Atom<'ast> {
    self.builder().atom(&format!("hot_{}", self.module.repr_name))
  }
}

impl<'any, 'ast> HmrAstBuilder<'any, 'ast> for ScopeHoistingFinalizer<'any, 'ast> {
  fn builder(&self) -> &oxc::ast::AstBuilder<'ast> {
    &self.snippet.builder
  }

  fn module(&self) -> &NormalModule {
    self.ctx.module
  }

  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Atom<'ast> {
    let name = self.canonical_name_for(self.ctx.module.namespace_object_ref);
    self.builder().atom(name)
  }

  fn alias_name_for_import_meta_hot(&self) -> ast::Atom<'ast> {
    let name =
      self.canonical_name_for(self.ctx.module.hmr_hot_ref.expect("HMR hot ref should be set"));
    self.builder().atom(name)
  }
}
