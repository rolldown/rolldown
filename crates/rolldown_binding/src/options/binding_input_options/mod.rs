// cSpell:disable

use crate::types::{binding_log::BindingLog, log_level::LogLevel};
use derivative::Derivative;
use napi::threadsafe_function::ThreadsafeFunction;
use napi_derive::napi;
use serde::Deserialize;

use self::{binding_input_item::BindingInputItem, binding_resolve_options::BindingResolveOptions};

use super::plugin::BindingPluginOrParallelJsPluginPlaceholder;

mod binding_input_item;
mod binding_resolve_options;

#[napi(object, object_to_js = false)]
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
  pub external: Option<ThreadsafeFunction<(String, Option<String>, bool), bool, false>>,
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
  pub plugins: Vec<BindingPluginOrParallelJsPluginPlaceholder>,
  pub resolve: Option<BindingResolveOptions>,
  // preserveEntrySignatures?: PreserveEntrySignaturesOption;
  // /** @deprecated Use the "preserveModules" output option instead. */
  // preserveModules?: boolean;
  // pub preserve_symlinks: bool,
  pub shim_missing_exports: Option<bool>,
  // strictDeprecations?: boolean;
  // pub treeshake: Option<bool>,
  // watch?: WatcherOptions | false;
  #[napi(ts_type = "'node' | 'browser' | 'neutral'")]
  pub platform: Option<String>,
  #[napi(ts_type = "'silent' | 'debug' | 'warn' | 'info'")]
  #[serde(skip_deserializing)]
  pub log_level: Option<LogLevel>,
  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(logLevel: 'debug' | 'warn' | 'info', log: BindingLog) => void")]
  pub on_log: BindingOnLog,
  // extra
  pub cwd: String,
  // pub builtins: BuiltinsOptions,
}

pub type BindingOnLog = Option<ThreadsafeFunction<(LogLevel, BindingLog), (), false>>;
