use napi::Either;
use oxc_transform_napi::TransformOptions;
use rolldown_plugin_transform::TransformPlugin;

use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingTransformPluginConfig {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  pub jsx_refresh_include: Option<Vec<BindingStringOrRegex>>,
  pub jsx_refresh_exclude: Option<Vec<BindingStringOrRegex>>,

  pub is_server_consumer: Option<bool>,
  pub runtime_resolve_base: Option<String>,

  pub jsx_inject: Option<String>,
  pub transform_options: Option<TransformOptions>,
}

impl From<BindingTransformPluginConfig> for TransformPlugin {
  fn from(value: BindingTransformPluginConfig) -> Self {
    let sourcemap = value.transform_options.as_ref().and_then(|v| v.sourcemap).unwrap_or(true);
    let transform_options = value.transform_options.map(|v| {
      let jsx = v.jsx.map(|jsx| match jsx {
        Either::A(jsx) => itertools::Either::Left(jsx),
        Either::B(jsx) => {
          let refresh = jsx.refresh.map(|refresh| match refresh {
            Either::A(refresh) => itertools::Either::Left(refresh),
            Either::B(refresh) => {
              itertools::Either::Right(rolldown_plugin_transform::ReactRefreshOptions {
                refresh_reg: refresh.refresh_reg,
                refresh_sig: refresh.refresh_sig,
                emit_full_signatures: refresh.emit_full_signatures,
              })
            }
          });
          itertools::Either::Right(rolldown_plugin_transform::JsxOptions {
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

      let target = v.target.map(|target| match target {
        Either::A(v) => itertools::Either::Left(v),
        Either::B(v) => itertools::Either::Right(v),
      });

      let decorator = v.decorator.map(|decorator| rolldown_plugin_transform::DecoratorOptions {
        legacy: decorator.legacy,
        emit_decorator_metadata: decorator.emit_decorator_metadata,
      });

      let typescript = v.typescript.map(|typescript| {
        let declaration = typescript.declaration.map(|declaration| {
          rolldown_plugin_transform::IsolatedDeclarationsOptions {
            strip_internal: declaration.strip_internal,
            sourcemap: declaration.sourcemap,
          }
        });
        let rewrite_import_extensions = typescript.rewrite_import_extensions.map(|v| match v {
          Either::A(v) => itertools::Either::Left(v),
          Either::B(v) => itertools::Either::Right(v),
        });

        rolldown_plugin_transform::TypeScriptOptions {
          declaration,
          jsx_pragma: typescript.jsx_pragma,
          jsx_pragma_frag: typescript.jsx_pragma_frag,
          only_remove_type_imports: typescript.only_remove_type_imports,
          allow_namespaces: typescript.allow_namespaces,
          allow_declare_fields: typescript.allow_declare_fields,
          rewrite_import_extensions,
        }
      });

      let assumptions = v.assumptions.map(|v| rolldown_plugin_transform::CompilerAssumptions {
        ignore_function_length: v.ignore_function_length,
        no_document_all: v.no_document_all,
        object_rest_no_symbols: v.object_rest_no_symbols,
        pure_getters: v.pure_getters,
        set_public_class_fields: v.set_public_class_fields,
      });

      rolldown_plugin_transform::TransformOptions {
        lang: v.lang,
        jsx,
        target,
        decorator,
        typescript,
        assumptions,
      }
    });

    Self {
      include: value.include.map(bindingify_string_or_regex_array).unwrap_or_default(),
      exclude: value.exclude.map(bindingify_string_or_regex_array).unwrap_or_default(),
      jsx_refresh_include: value
        .jsx_refresh_include
        .map(bindingify_string_or_regex_array)
        .unwrap_or_default(),
      jsx_refresh_exclude: value
        .jsx_refresh_exclude
        .map(bindingify_string_or_regex_array)
        .unwrap_or_default(),
      jsx_inject: value.jsx_inject,
      is_server_consumer: value.is_server_consumer.unwrap_or(true),
      sourcemap,
      transform_options: transform_options.unwrap_or_default(),
    }
  }
}
