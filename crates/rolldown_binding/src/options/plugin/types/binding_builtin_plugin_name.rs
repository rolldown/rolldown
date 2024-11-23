use napi_derive::napi;
use serde::Deserialize;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Deserialize)]
#[napi]
pub enum BindingBuiltinPluginName {
  WasmHelperPlugin,
  ImportGlobPlugin,
  DynamicImportVarsPlugin,
  ModulePreloadPolyfillPlugin,
  ManifestPlugin,
  LoadFallbackPlugin,
  TransformPlugin,
  WasmFallbackPlugin,
  AliasPlugin,
  JsonPlugin,
  BuildImportAnalysisPlugin,
  ReplacePlugin,
  ViteResolvePlugin,
}
