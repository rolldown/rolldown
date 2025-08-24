use oxc_transform_napi::TransformOptions;
use rolldown_common::{
  BundlerTransformOptions, CompilerAssumptions, DecoratorOptions, Either,
  IsolatedDeclarationsOptions, JsxOptions, PluginsOptions, ReactRefreshOptions,
  StyledComponentsOptions, TypeScriptOptions,
};

pub fn normalize_binding_transform_options(options: TransformOptions) -> BundlerTransformOptions {
  let jsx = options.jsx.map(|jsx| match jsx {
    napi::Either::A(jsx) => Either::Left(jsx),
    napi::Either::B(jsx) => {
      let refresh = jsx.refresh.map(|refresh| match refresh {
        napi::Either::A(refresh) => Either::Left(refresh),
        napi::Either::B(refresh) => Either::Right(ReactRefreshOptions {
          refresh_reg: refresh.refresh_reg,
          refresh_sig: refresh.refresh_sig,
          emit_full_signatures: refresh.emit_full_signatures,
        }),
      });
      Either::Right(JsxOptions {
        runtime: jsx.runtime,
        development: jsx.development,
        throw_if_namespace: jsx.throw_if_namespace,
        pure: jsx.pure,
        import_source: jsx.import_source,
        pragma: jsx.pragma,
        pragma_frag: jsx.pragma_frag,
        use_built_ins: jsx.use_built_ins,
        use_spread: jsx.use_spread,
        refresh,
      })
    }
  });

  let target = options.target.map(|target| match target {
    napi::Either::A(v) => Either::Left(v),
    napi::Either::B(v) => Either::Right(v),
  });

  let decorator = options.decorator.map(|decorator| DecoratorOptions {
    legacy: decorator.legacy,
    emit_decorator_metadata: decorator.emit_decorator_metadata,
  });

  let typescript = options.typescript.map(|typescript| {
    let declaration = typescript.declaration.map(|declaration| IsolatedDeclarationsOptions {
      strip_internal: declaration.strip_internal,
      sourcemap: declaration.sourcemap,
    });
    let rewrite_import_extensions = typescript.rewrite_import_extensions.map(|v| match v {
      napi::Either::A(v) => Either::Left(v),
      napi::Either::B(v) => Either::Right(v),
    });

    TypeScriptOptions {
      declaration,
      jsx_pragma: typescript.jsx_pragma,
      jsx_pragma_frag: typescript.jsx_pragma_frag,
      only_remove_type_imports: typescript.only_remove_type_imports,
      allow_namespaces: typescript.allow_namespaces,
      allow_declare_fields: typescript.allow_declare_fields,
      remove_class_fields_without_initializer: typescript.remove_class_fields_without_initializer,
      rewrite_import_extensions,
    }
  });

  let assumptions = options.assumptions.map(|v| CompilerAssumptions {
    ignore_function_length: v.ignore_function_length,
    no_document_all: v.no_document_all,
    object_rest_no_symbols: v.object_rest_no_symbols,
    pure_getters: v.pure_getters,
    set_public_class_fields: v.set_public_class_fields,
  });

  let plugins = options.plugins.map(|v| PluginsOptions {
    styled_components: v.styled_components.map(|s| StyledComponentsOptions {
      display_name: s.display_name,
      file_name: s.file_name,
      ssr: s.ssr,
      transpile_template_literals: s.transpile_template_literals,
      minify: s.minify,
      css_prop: s.css_prop,
      pure: s.pure,
      namespace: s.namespace,
      meaningless_file_names: s.meaningless_file_names,
      top_level_import_paths: s.top_level_import_paths,
    }),
  });

  BundlerTransformOptions { jsx, target, decorator, typescript, assumptions, plugins }
}
