use oxc::allocator::GetAllocator;
use oxc::ast::builder::AstBuilder;
use oxc::{ast::ast, span::SPAN};
use rolldown_common::NormalModule;
use rolldown_ecmascript::CJS_MODULE_REF;
#[cfg(feature = "experimental")]
use rolldown_ecmascript::CJS_ROLLDOWN_MODULE_REF;
use rolldown_ecmascript_utils::ExpressionFactoryExt as _;

#[cfg(feature = "experimental")]
use crate::hmr::hmr_ast_finalizer::HmrAstFinalizer;
use crate::module_finalizers::ScopeHoistingFinalizer;

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
  /// The HMR/lazy path wraps each module body in `createEsmInitializer(id, function () { … })`,
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
  /// `createEsmInitializer(id, function (__rolldown_module_id__) { … })`
  /// (or `createCjsInitializer(id, function (exports, module, __rolldown_module_id__) { … })`),
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

const LAZY_PROXY_QUERY: &str = "?rolldown-lazy=1";

/// `__rolldown_runtime__.requestLazy("<stable_real_id>", () => import(`/@vite/lazy?id=…&clientId=…`))`
///
/// The one shape both codegen paths emit for a lazy boundary, so a boundary never becomes a
/// chunk the browser fetches.
pub fn create_request_lazy_call<'ast, B>(
  proxy_module_id: &str,
  stable_proxy_id: &str,
  builder: &B,
) -> ast::Expression<'ast>
where
  B: oxc::ast::builder::GetAstBuilder<'ast> + GetAllocator<'ast>,
{
  let encode_call = ast::Expression::new_call_expression(
    SPAN,
    ast::Expression::new_identifier(SPAN, "encodeURIComponent", builder),
    None,
    [ast::Argument::new_string_literal(
      SPAN,
      ast::Str::from_str_in(proxy_module_id, builder),
      None,
      builder,
    )],
    false,
    builder,
  );

  let url_expr = {
    let quasis = oxc::allocator::Vec::from_iter_in(
      [
        ast::TemplateElement::new(
          SPAN,
          ast::TemplateElementValue { raw: ast::Str::from("/@vite/lazy?id="), cooked: None },
          false,
          builder,
        ),
        ast::TemplateElement::new(
          SPAN,
          ast::TemplateElementValue { raw: ast::Str::from("&clientId="), cooked: None },
          false,
          builder,
        ),
        ast::TemplateElement::new(
          SPAN,
          ast::TemplateElementValue { raw: ast::Str::from(""), cooked: None },
          true,
          builder,
        ),
      ],
      builder,
    );
    let expressions = oxc::allocator::Vec::from_iter_in(
      [
        encode_call,
        ast::Expression::new_member_access_expr("__rolldown_runtime__", "clientId", builder),
      ],
      builder,
    );
    ast::Expression::new_template_literal(SPAN, quasis, expressions, builder)
  };

  // () => import(`/@vite/lazy?...`)
  let fetch_chunk = ast::Expression::new_arrow_function_expression(
    SPAN,
    /* async */ false,
    None,
    ast::FormalParameters::boxed(
      SPAN,
      ast::FormalParameterKind::ArrowFormalParameters,
      [],
      None,
      builder,
    ),
    None,
    ast::ArrowFunctionBody::from(ast::Expression::new_import_expression(
      SPAN, url_expr, None, None, builder,
    )),
    builder,
  );

  ast::Expression::new_call_expression(
    SPAN,
    ast::Expression::new_identifier(SPAN, "__rolldown_runtime__.requestLazy", builder),
    None,
    [
      // Stripping the marker recovers the id the delivered chunk registers a factory under.
      ast::Argument::new_string_literal(
        SPAN,
        ast::Str::from_str_in(
          stable_proxy_id.strip_suffix(LAZY_PROXY_QUERY).unwrap_or(stable_proxy_id),
          builder,
        ),
        None,
        builder,
      ),
      ast::Argument::from(fetch_chunk),
    ],
    false,
    builder,
  )
}
