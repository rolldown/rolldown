use napi_derive::napi;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[napi]
pub enum BindingBuiltinPluginName {
  WasmHelper,
  ImportGlob,
  DynamicImportVars,
  ModulePreloadPolyfill,
  Manifest,
  LoadFallback,
  Transform,
  WasmFallback,
  Alias,
  Json,
  BuildImportAnalysis,
  Replace,
  ViteResolve,
}
