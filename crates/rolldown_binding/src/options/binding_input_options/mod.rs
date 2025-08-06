mod binding_debug_options;
mod binding_experimental_options;
mod binding_input_item;
mod binding_make_absolute_externals_relative;
mod binding_optimization;
mod binding_resolve_options;
mod binding_treeshake;
mod binding_watch_option;

pub mod binding_inject_import;
pub mod binding_jsx;

use binding_debug_options::BindingDebugOptions;
use binding_make_absolute_externals_relative::BindingMakeAbsoluteExternalsRelative;
use binding_optimization::BindingOptimization;
use derive_more::Debug;
use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;
use rustc_hash::FxBuildHasher;
use std::collections::HashMap;

use binding_inject_import::BindingInjectImport;
use binding_input_item::BindingInputItem;
use binding_jsx::BindingJsx;
use binding_resolve_options::BindingResolveOptions;
use binding_watch_option::BindingWatchOption;

use super::plugin::BindingPluginOrParallelJsPluginPlaceholder;
use crate::generated::binding_checks_options;
use crate::types::defer_sync_scan_data::BindingDeferSyncScanData;
use crate::types::preserve_entry_signatures::BindingPreserveEntrySignatures;
use crate::types::{
  binding_log::BindingLog, binding_log_level::BindingLogLevel, js_callback::JsCallback,
};

pub type BindingOnLog = Option<JsCallback<FnArgs<(String, BindingLog)>, ()>>;

#[napi(object, object_to_js = false)]
#[derive(Default, Debug)]
pub struct BindingInputOptions<'env> {
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
  pub external: Option<JsCallback<FnArgs<(String, Option<String>, bool)>, bool>>,
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
  pub plugins: Vec<BindingPluginOrParallelJsPluginPlaceholder<'env>>,
  pub resolve: Option<BindingResolveOptions>,
  // preserveEntrySignatures?: PreserveEntrySignaturesOption;
  // /** @deprecated Use the "preserveModules" output option instead. */
  // preserveModules?: boolean;
  // pub preserve_symlinks: bool,
  pub shim_missing_exports: Option<bool>,
  // strictDeprecations?: boolean;
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
  #[debug(skip)]
  pub transform: Option<oxc_transform_napi::TransformOptions>,
  pub watch: Option<BindingWatchOption>,
  pub keep_names: Option<bool>,
  pub checks: Option<binding_checks_options::BindingChecksOptions>,
  #[debug(skip)]
  #[napi(ts_type = "undefined | (() => BindingDeferSyncScanData[])")]
  pub defer_sync_scan_data: Option<JsCallback<(), Vec<BindingDeferSyncScanData>>>,
  pub make_absolute_externals_relative: Option<BindingMakeAbsoluteExternalsRelative>,
  pub debug: Option<BindingDebugOptions>,
  #[debug(skip)]
  #[napi(ts_type = "() => void")]
  pub invalidate_js_side_cache: Option<JsCallback>,
  #[debug(skip)]
  #[napi(ts_type = "(id: string, success: boolean) => void")]
  pub mark_module_loaded: Option<JsCallback<FnArgs<(String, bool)>>>,
  pub preserve_entry_signatures: Option<BindingPreserveEntrySignatures>,
  pub optimization: Option<BindingOptimization>,
}
