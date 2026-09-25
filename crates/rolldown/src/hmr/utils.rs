use oxc::allocator::GetAllocator;
use oxc::ast::builder::AstBuilder;
use oxc::{ast::ast, span::SPAN};
use rolldown_common::{IndexModules, NormalModule};
use rolldown_ecmascript::CJS_MODULE_REF;
#[cfg(feature = "experimental")]
use rolldown_ecmascript::CJS_ROLLDOWN_MODULE_REF;
use rolldown_ecmascript_utils::ExpressionExt;

#[cfg(feature = "experimental")]
use crate::hmr::hmr_ast_finalizer::HmrAstFinalizer;
use crate::module_finalizers::ScopeHoistingFinalizer;

/// The runtime matches accepted deps by stable id, so each recorded dep is rewritten to it.
pub fn rewrite_hot_accept_deps<'ast>(
  call_expr: &mut ast::CallExpression<'ast>,
  module: &NormalModule,
  modules: &IndexModules,
  builder: &AstBuilder<'ast>,
) {
  if !call_expr.callee.is_import_meta_hot_accept() {
    return;
  }
  let rewrite = |expr: &mut ast::Expression<'ast>| {
    let Some(request) = expr.as_static_module_request() else { return };
    let Some(record_idx) =
      module.hmr_info.module_request_to_import_record_idx.get(request.as_str())
    else {
      return;
    };
    let Some(module_idx) = module.import_records[*record_idx].resolved_module else { return };
    *expr = ast::Expression::new_string_literal(
      SPAN,
      ast::Str::from_str_in(modules[module_idx].stable_id(), builder),
      None,
      builder,
    );
  };
  match call_expr.arguments.first_mut() {
    Some(ast::Argument::ArrayExpression(array_expression)) => {
      array_expression
        .elements
        .iter_mut()
        .filter_map(|element| element.as_expression_mut())
        .for_each(rewrite);
    }
    Some(argument) => {
      if let Some(expr) = argument.as_expression_mut() {
        rewrite(expr);
      }
    }
    None => {}
  }
}

#[cfg(feature = "experimental")]
pub static MODULE_EXPORTS_NAME_FOR_ESM: &str = "__rolldown_exports__";
#[cfg(feature = "experimental")]
pub static MODULE_ID_PARAM_FOR_HMR: &str = "__rolldown_module_id__";

pub trait HmrAstBuilder<'any, 'ast> {
  fn builder(&self) -> AstBuilder<'ast>;

  fn module(&self) -> &NormalModule;

  // `${ns_name}` in `var ${ns_name} = ...`
  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Str<'ast>;

  // `$hot_name` in var `$hot_name = __rolldown_runtime__.createModuleHotContext($stable_id);`
  fn alias_name_for_import_meta_hot(&self) -> ast::Str<'ast>;

  fn cjs_module_name() -> &'static str;

  /// How to refer to the current module id at the emission site.
  ///
  /// The HMR/lazy path wraps each module body in `registerFactory(id, function () { … })`,
  /// so inside the body the id is available as an identifier (`__rolldown_module_id__`) passed in
  /// by the runtime. The main-bundle path has no such wrapper, so it still needs to emit the
  /// stable id as a string literal.
  fn module_id_argument(&self) -> ast::Argument<'ast> {
    ast::Argument::new_string_literal(
      SPAN,
      ast::Str::from_str_in(&self.module().stable_id, &self.builder()),
      None,
      &self.builder(),
    )
  }

  /// `__rolldown_runtime__.registerModule(moduleId[, module])`
  fn create_register_module_stmt(&self) -> ast::Statement<'ast> {
    let module_exports = match self.module().exports_kind {
      rolldown_common::ExportsKind::Esm => {
        let binding_name_for_namespace_object_ref_atom =
          self.binding_name_for_namespace_object_ref_atom();
        let namespace_object_ref_expr = ast::Expression::new_identifier(
          SPAN,
          binding_name_for_namespace_object_ref_atom,
          &self.builder(),
        );

        // { exports: namespace }
        Some(ast::Argument::new_object_expression(
          SPAN,
          [ast::ObjectPropertyKind::new_object_property(
            SPAN,
            ast::PropertyKind::Init,
            ast::PropertyKey::new_static_identifier(SPAN, "exports", &self.builder()),
            namespace_object_ref_expr,
            true,
            false,
            false,
            &self.builder(),
          )],
          &self.builder(),
        ))
      }
      rolldown_common::ExportsKind::CommonJs => {
        // `module`
        Some(ast::Argument::new_identifier(SPAN, Self::cjs_module_name(), &self.builder()))
      }
      rolldown_common::ExportsKind::None => None,
    };

    // __rolldown_runtime__.registerModule(moduleId[, module])
    // moduleId is either `__rolldown_module_id__` (HMR/lazy path) or the stable-id
    // string literal (main-bundle path).
    let register_call = ast::Expression::new_call_expression(
      SPAN,
      ast::Expression::new_identifier(SPAN, "__rolldown_runtime__.registerModule", &self.builder()),
      None,
      oxc::allocator::Vec::from_iter_in(
        std::iter::once(self.module_id_argument()).chain(module_exports),
        &self.builder(),
      ),
      false,
      &self.builder(),
    );

    ast::Statement::new_expression_statement(SPAN, register_call, &self.builder())
  }

  /// `var $hot_name = __rolldown_runtime__.createModuleHotContext($stable_id);`
  fn create_module_hot_context_initializer_stmt(&self) -> ast::Statement<'ast> {
    // var $hot_name = __rolldown_runtime__.createModuleHotContext($stable_id);
    // Use stable module ID for consistent lookup
    ast::Statement::new_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Const,
      oxc::allocator::Vec::from_value_in(
        // var $hot_name
        ast::VariableDeclarator::new(
          SPAN,
          ast::BindingPattern::new_binding_identifier(
            SPAN,
            self.alias_name_for_import_meta_hot(),
            &self.builder(),
          ),
          None,
          // __rolldown_runtime__.createModuleHotContext($stable_id)
          Some(ast::Expression::new_call_expression(
            SPAN,
            ast::Expression::new_identifier(
              SPAN,
              "__rolldown_runtime__.createModuleHotContext",
              &self.builder(),
            ),
            None,
            [self.module_id_argument()],
            false,
            &self.builder(),
          )),
          false,
          &self.builder(),
        ),
        &self.builder(),
      ),
      false,
      &self.builder(),
    )
  }
}

#[cfg(feature = "experimental")]
impl<'any, 'ast> HmrAstBuilder<'any, 'ast> for HmrAstFinalizer<'any, 'ast> {
  fn builder(&self) -> AstBuilder<'ast> {
    AstBuilder::new(self.ast_builder.allocator())
  }

  fn module(&self) -> &NormalModule {
    self.module
  }

  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Str<'ast> {
    ast::Str::from(MODULE_EXPORTS_NAME_FOR_ESM)
  }

  fn alias_name_for_import_meta_hot(&self) -> ast::Str<'ast> {
    ast::Str::from_str_in(&format!("hot_{}", self.module.repr_name), &self.builder())
  }

  fn cjs_module_name() -> &'static str {
    CJS_ROLLDOWN_MODULE_REF
  }

  /// HMR/lazy path: each module body is wrapped in
  /// `registerFactory(id, function (__rolldown_module_id__) { … })`,
  /// so the id is in lexical scope as a parameter.
  fn module_id_argument(&self) -> ast::Argument<'ast> {
    ast::Argument::new_identifier(SPAN, MODULE_ID_PARAM_FOR_HMR, &self.builder())
  }
}

impl<'any, 'ast> HmrAstBuilder<'any, 'ast> for ScopeHoistingFinalizer<'any, 'ast> {
  fn builder(&self) -> AstBuilder<'ast> {
    AstBuilder::new(self.ast_builder.allocator())
  }

  fn module(&self) -> &NormalModule {
    self.ctx.module
  }

  fn binding_name_for_namespace_object_ref_atom(&self) -> ast::Str<'ast> {
    let name = self.canonical_name_for(self.ctx.module.namespace_object_ref);
    ast::Str::from_str_in(name, &self.builder())
  }

  fn alias_name_for_import_meta_hot(&self) -> ast::Str<'ast> {
    let name =
      self.canonical_name_for(self.ctx.module.hmr_hot_ref.expect("HMR hot ref should be set"));
    ast::Str::from_str_in(name, &self.builder())
  }

  fn cjs_module_name() -> &'static str {
    CJS_MODULE_REF
  }
}
