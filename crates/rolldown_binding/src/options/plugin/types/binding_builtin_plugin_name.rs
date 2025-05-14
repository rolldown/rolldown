use napi_derive::napi;

#[derive(Debug)]
#[napi(string_enum)]
pub enum BindingBuiltinPluginName {
  #[napi(value = "builtin:wasm-helper")]
  WasmHelper,
  #[napi(value = "builtin:import-glob")]
  ImportGlob,
  #[napi(value = "builtin:dynamic-import-vars")]
  DynamicImportVars,
  #[napi(value = "builtin:module-preload-polyfill")]
  ModulePreloadPolyfill,
  #[napi(value = "builtin:manifest")]
  Manifest,
  #[napi(value = "builtin:load-fallback")]
  LoadFallback,
  #[napi(value = "builtin:transform")]
  Transform,
  #[napi(value = "builtin:wasm-fallback")]
  WasmFallback,
  #[napi(value = "builtin:alias")]
  Alias,
  #[napi(value = "builtin:json")]
  Json,
  #[napi(value = "builtin:build-import-analysis")]
  BuildImportAnalysis,
  #[napi(value = "builtin:replace")]
  Replace,
  #[napi(value = "builtin:vite-resolve")]
  ViteResolve,
  #[napi(value = "builtin:module-federation")]
  ModuleFederation,
  #[napi(value = "builtin:isolated-declaration")]
  IsolatedDeclaration,
  #[napi(value = "builtin:report")]
  Report,
  #[napi(value = "builtin:asset")]
  Asset,
}
