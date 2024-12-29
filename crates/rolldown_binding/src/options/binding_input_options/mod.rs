mod binding_checks_options;
mod binding_experimental_options;
pub mod binding_inject_import;
mod binding_input_item;
mod binding_jsx;
mod binding_resolve_options;
mod binding_treeshake;
mod binding_watch_option;

use derive_more::Debug;
use napi_derive::napi;
use rustc_hash::FxBuildHasher;
use std::collections::HashMap;

use binding_inject_import::BindingInjectImport;
use binding_input_item::BindingInputItem;
use binding_jsx::BindingJsx;
use binding_resolve_options::BindingResolveOptions;
use binding_watch_option::BindingWatchOption;

use super::plugin::BindingPluginOrParallelJsPluginPlaceholder;
use crate::types::{
  binding_log::BindingLog, binding_log_level::BindingLogLevel, js_callback::JsCallback,
};

pub type BindingOnLog = Option<JsCallback<(String, BindingLog), ()>>;

#[napi(object, object_to_js = false)]
#[derive(Default, Debug)]
pub struct BindingInputOptions {
  // Not going to be supported
  // @deprecated Use the "inlineDynamicImports" output option instead.
  // inlineDynamicImports?: boolean;

  // acorn?: Record<string, unknown>;
  // acornInjectPlugins?: (() => unknown)[] | (() => unknown);
  // cache?: false | RollupCache;
  // context?: string;
  // experimentalCacheExpiry?: number;
  #[debug(skip)]
  #[napi(
    ts_type = "undefined | ((source: string, importer: string | undefined, isResolved: boolean) => boolean)"
  )]
  pub external: Option<JsCallback<(String, Option<String>, bool), bool>>,
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
  #[napi(ts_type = "(BindingBuiltinPlugin | BindingPluginOptions | undefined)[]")]
  pub plugins: Vec<BindingPluginOrParallelJsPluginPlaceholder>,
  pub resolve: Option<BindingResolveOptions>,
  // preserveEntrySignatures?: PreserveEntrySignaturesOption;
  // /** @deprecated Use the "preserveModules" output option instead. */
  // preserveModules?: boolean;
  // pub preserve_symlinks: bool,
  pub shim_missing_exports: Option<bool>,
  // strictDeprecations?: boolean;
  // pub treeshake: Option<bool>,
  #[napi(ts_type = "'node' | 'browser' | 'neutral'")]
  pub platform: Option<String>,
  pub log_level: BindingLogLevel,
  #[debug(skip)]
  #[napi(ts_type = "(logLevel: 'debug' | 'warn' | 'info', log: BindingLog) => void")]
  pub on_log: BindingOnLog,
  // extra
  pub cwd: String,
  // pub builtins: BuiltinsOptions,
  pub treeshake: Option<binding_treeshake::BindingTreeshake>,

  pub module_types: Option<HashMap<String, String, FxBuildHasher>>,
  pub define: Option<Vec<(/* Target to be replaced */ String, /* Replacement */ String)>>,
  pub drop_labels: Option<Vec<String>>,
  #[napi(ts_type = "Array<BindingInjectImportNamed | BindingInjectImportNamespace>")]
  pub inject: Option<Vec<BindingInjectImport>>,
  pub experimental: Option<binding_experimental_options::BindingExperimentalOptions>,
  pub profiler_names: Option<bool>,
  #[debug(skip)]
  pub jsx: Option<BindingJsx>,
  pub watch: Option<BindingWatchOption>,
  pub keep_names: Option<bool>,
  pub checks: Option<binding_checks_options::BindingChecksOptions>,
}
