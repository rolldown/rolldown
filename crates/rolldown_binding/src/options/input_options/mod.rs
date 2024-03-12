// cSpell:disable
use std::{collections::HashMap, fmt::Debug, path::PathBuf};

use crate::{
  options::plugin::JsAdapterPlugin,
  utils::{napi_error_ext::NapiErrorExt, JsCallback},
};
use derivative::Derivative;
use napi::JsFunction;
use napi_derive::napi;

use serde::Deserialize;

use super::plugin::PluginOptions;

#[napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct InputItem {
  pub name: Option<String>,
  pub import: String,
}

impl From<InputItem> for rolldown::InputItem {
  fn from(value: InputItem) -> Self {
    Self { name: value.name, import: value.import }
  }
}

pub type ExternalFn = JsCallback<(String, Option<String>, bool), bool>;

#[napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResolveOptions {
  pub alias: Option<HashMap<String, Vec<String>>>,
  pub alias_fields: Option<Vec<Vec<String>>>,
  pub condition_names: Option<Vec<String>>,
  pub exports_fields: Option<Vec<Vec<String>>>,
  pub extensions: Option<Vec<String>>,
  pub main_fields: Option<Vec<String>>,
  pub main_files: Option<Vec<String>>,
  pub modules: Option<Vec<String>>,
  pub symlinks: Option<bool>,
}

impl From<ResolveOptions> for rolldown_resolver::ResolverOptions {
  fn from(value: ResolveOptions) -> Self {
    Self {
      alias: value.alias.map(|alias| alias.into_iter().collect::<Vec<_>>()),
      alias_fields: value.alias_fields,
      condition_names: value.condition_names,
      exports_fields: value.exports_fields,
      extensions: value.extensions,
      main_fields: value.main_fields,
      main_files: value.main_files,
      modules: value.modules,
      symlinks: value.symlinks,
    }
  }
}

#[napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct InputOptions {
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
  pub input: Vec<InputItem>,
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
  pub resolve: Option<ResolveOptions>,
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
impl From<InputOptions>
  for (napi::Result<rolldown::InputOptions>, napi::Result<Vec<rolldown_plugin::BoxPlugin>>)
{
  fn from(value: InputOptions) -> Self {
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
