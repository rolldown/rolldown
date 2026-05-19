use oxc::{
  ast::{NONE, ast},
  span::SPAN,
};
use rolldown_common::NormalModule;
use rolldown_ecmascript::{CJS_MODULE_REF, CJS_ROLLDOWN_MODULE_REF};

use crate::{hmr::hmr_ast_finalizer::HmrAstFinalizer, module_finalizers::ScopeHoistingFinalizer};

pub static MODULE_EXPORTS_NAME_FOR_ESM: &str = "__rolldown_exports__";
pub static MODULE_ID_PARAM_FOR_HMR: &str = "__rolldown_module_id__";

pub trait HmrAstBuilder<'any, 'ast> {
  fn builder(&self) -> &oxc::ast::AstBuilder<'ast>;

  fn module(&self) -> &NormalModule;

  // `${ns_name}` in `var ${ns_name} = ...`
  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Str<'ast>;

  // `$hot_name` in var `$hot_name = __rolldown_runtime__.createModuleHotContext($stable_id);`
  fn alias_name_for_import_meta_hot(&self) -> ast::Str<'ast>;

  fn cjs_module_name() -> &'static str;

  /// How to refer to the current module id at the emission site.
  ///
  /// The HMR/lazy path wraps each module body in `createEsmInitializer(id, function () { … })`,
  /// so inside the body the id is available as an identifier (`__rolldown_module_id__`) passed in
  /// by the runtime. The main-bundle path has no such wrapper, so it still needs to emit the
  /// stable id as a string literal.
  fn module_id_argument(&self) -> ast::Argument<'ast> {
    ast::Argument::StringLiteral(self.builder().alloc_string_literal(
      SPAN,
      self.builder().str(&self.module().stable_id),
      None,
    ))
  }

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
          self.builder().alloc_identifier_reference(SPAN, Self::cjs_module_name()),
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
    // moduleId is either `__rolldown_module_id__` (HMR/lazy path) or the stable-id
    // string literal (main-bundle path).
    let arguments = self.builder().vec_from_array([self.module_id_argument(), module_exports]);

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
    // Use stable module ID for consistent lookup
    ast::Statement::VariableDeclaration(
      self.builder().alloc_variable_declaration(
        SPAN,
        ast::VariableDeclarationKind::Const,
        self.builder().vec1(
          // var $hot_name
          self.builder().variable_declarator(
            SPAN,
            ast::VariableDeclarationKind::Const,
            self
              .builder()
              .binding_pattern_binding_identifier(SPAN, self.alias_name_for_import_meta_hot()),
            NONE,
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
                self.builder().vec1(self.module_id_argument()),
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

  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Str<'ast> {
    self.builder().str(MODULE_EXPORTS_NAME_FOR_ESM)
  }

  fn alias_name_for_import_meta_hot(&self) -> ast::Str<'ast> {
    self.builder().str(&format!("hot_{}", self.module.repr_name))
  }

  fn cjs_module_name() -> &'static str {
    CJS_ROLLDOWN_MODULE_REF
  }

  /// HMR/lazy path: each module body is wrapped in
  /// `createEsmInitializer(id, function (__rolldown_module_id__) { … })`
  /// (or `createCjsInitializer(id, function (exports, module, __rolldown_module_id__) { … })`),
  /// so the id is in lexical scope as a parameter.
  fn module_id_argument(&self) -> ast::Argument<'ast> {
    ast::Argument::Identifier(
      self.builder().alloc_identifier_reference(SPAN, MODULE_ID_PARAM_FOR_HMR),
    )
  }
}

impl<'any, 'ast> HmrAstBuilder<'any, 'ast> for ScopeHoistingFinalizer<'any, 'ast> {
  fn builder(&self) -> &oxc::ast::AstBuilder<'ast> {
    &self.snippet.builder
  }

  fn module(&self) -> &NormalModule {
    self.ctx.module
  }

  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Str<'ast> {
    let name = self.canonical_name_for(self.ctx.module.namespace_object_ref);
    self.builder().str(name)
  }

  fn alias_name_for_import_meta_hot(&self) -> ast::Str<'ast> {
    let name =
      self.canonical_name_for(self.ctx.module.hmr_hot_ref.expect("HMR hot ref should be set"));
    self.builder().str(name)
  }

  fn cjs_module_name() -> &'static str {
    CJS_MODULE_REF
  }
}
