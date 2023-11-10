use std::path::PathBuf;
mod plugin;
mod plugin_adapter;
use napi_derive::napi;

use serde::Deserialize;

use crate::options::input_options::plugin_adapter::JsAdapterPlugin;

use self::plugin::PluginOptions;

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

#[napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct InputOptions {
  // Not going to be supported
  // @deprecated Use the "inlineDynamicImports" output option instead.
  // inlineDynamicImports?: boolean;

  // acorn?: Record<string, unknown>;
  // acornInjectPlugins?: (() => unknown)[] | (() => unknown);
  // cache?: false | RollupCache;
  // context?: string;sssssssssss
  // experimentalCacheExpiry?: number;
  // pub external: ExternalOption,
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

pub fn resolve_input_options(
  opts: InputOptions,
) -> napi::Result<(rolldown::InputOptions, Vec<Box<dyn rolldown::Plugin>>)> {
  let cwd = PathBuf::from(opts.cwd.clone());
  assert!(cwd != PathBuf::from("/"), "{opts:#?}");

  let plugins =
    opts.plugins.into_iter().map(JsAdapterPlugin::new_boxed).collect::<napi::Result<Vec<_>>>()?;

  Ok((
    rolldown::InputOptions {
      input: Some(opts.input.into_iter().map(Into::into).collect::<Vec<_>>()),
      cwd: Some(cwd),
    },
    plugins,
  ))
}
