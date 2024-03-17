// cSpell:disable
use std::path::PathBuf;

use crate::{
  options::plugin::JsAdapterPlugin,
  utils::{napi_error_ext::NapiErrorExt, JsCallback},
};
use derivative::Derivative;
use napi::JsFunction;
use napi_derive::napi;

use serde::Deserialize;

use self::{binding_input_item::BindingInputItem, binding_resolve_options::BindingResolveOptions};

use super::plugin::PluginOptions;

mod binding_input_item;
mod binding_resolve_options;

pub type ExternalFn = JsCallback<(String, Option<String>, bool), bool>;

#[napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingInputOptions {
  // Not going to be supported
  // @deprecated Use the "inlineDynamicImports" output option instead.
  // inlineDynamicImports?: boolean;

  // acorn?: Record<string, unknown>;
  // acornInjectPlugins?: (() => unknown)[] | (() => unknown);
  // cache?: false | RollupCache;
  // context?: string;sssssssssss
  // experimentalCacheExpiry?: number;
  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "undefined | ((source: string, importer: string | undefined, isResolved: boolean) => boolean)"
  )]
  pub external: Option<JsFunction>,
  pub input: Vec<BindingInputItem>,
  // makeAbsoluteExternalsRelative?: boolean | 'ifRelativeSource';
  // /** @deprecated Use the "manualChunks" output option instead. */
  // manualChunks?: ManualChunksOption;
  // maxParallelFileOps?: number;
  // /** @deprecated Use the "maxParallelFileOps" option instead. */
  // maxParallelFileReads?: number;
  // moduleContext?: ((id: string) => string | null | void) | { [id: string]: string };
  // onwarn?: WarningHandlerWithDefault;
  // perf?: boolean;
  pub plugins: Vec<PluginOptions>,
  pub resolve: Option<BindingResolveOptions>,
  // preserveEntrySignatures?: PreserveEntrySignaturesOption;
  // /** @deprecated Use the "preserveModules" output option instead. */
  // preserveModules?: boolean;
  // pub preserve_symlinks: bool,
  // pub shim_missing_exports: bool,
  // strictDeprecations?: boolean;
  // pub treeshake: Option<bool>,
  // watch?: WatcherOptions | false;

  // extra
  pub cwd: String,
  // pub builtins: BuiltinsOptions,
}

#[allow(clippy::redundant_closure_for_method_calls)]
impl From<BindingInputOptions>
  for (napi::Result<rolldown::InputOptions>, napi::Result<Vec<rolldown_plugin::BoxPlugin>>)
{
  fn from(value: BindingInputOptions) -> Self {
    let cwd = PathBuf::from(value.cwd.clone());
    assert!(cwd != PathBuf::from("/"), "{value:#?}");

    let external = if let Some(js_fn) = value.external {
      match ExternalFn::new(&js_fn) {
        Err(e) => return (Err(e), Ok(vec![])),
        Ok(external_fn) => {
          let cb = Box::new(external_fn);
          rolldown::External::Fn(Box::new(move |source, importer, is_resolved| {
            let ts_fn = Box::clone(&cb);
            Box::pin(async move {
              ts_fn
                .call_async((source, importer, is_resolved))
                .await
                .map_err(|e| e.into_bundle_error())
            })
          }))
        }
      }
    } else {
      rolldown::External::default()
    };

    (
      Ok(rolldown::InputOptions {
        input: value.input.into_iter().map(Into::into).collect::<Vec<_>>(),
        cwd,
        external,
        treeshake: true,
        resolve: value.resolve.map(Into::into),
      }),
      value.plugins.into_iter().map(JsAdapterPlugin::new_boxed).collect::<napi::Result<Vec<_>>>(),
    )
  }
}
