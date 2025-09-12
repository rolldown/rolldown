use napi_derive::napi;

#[derive(Debug)]
#[napi(string_enum)]
pub enum BindingBuiltinPluginName {
  #[napi(value = "builtin:alias")]
  Alias,
  #[napi(value = "builtin:asset")]
  Asset,
  #[napi(value = "builtin:asset-import-meta-url")]
  AssetImportMetaUrl,
  #[napi(value = "builtin:build-import-analysis")]
  BuildImportAnalysis,
  #[napi(value = "builtin:dynamic-import-vars")]
  DynamicImportVars,
  #[napi(value = "builtin:import-glob")]
  ImportGlob,
  #[napi(value = "builtin:isolated-declaration")]
  IsolatedDeclaration,
  #[napi(value = "builtin:json")]
  Json,
  #[napi(value = "builtin:load-fallback")]
  LoadFallback,
  #[napi(value = "builtin:manifest")]
  Manifest,
  #[napi(value = "builtin:module-preload-polyfill")]
  ModulePreloadPolyfill,
  #[napi(value = "builtin:oxc-runtime")]
  OxcRuntime,
  #[napi(value = "builtin:react-refresh-wrapper")]
  ReactRefreshWrapper,
  #[napi(value = "builtin:reporter")]
  Report,
  #[napi(value = "builtin:replace")]
  Replace,
  #[napi(value = "builtin:esm-external-require")]
  RequireToImport,
  #[napi(value = "builtin:transform")]
  Transform,
  #[napi(value = "builtin:vite-resolve")]
  ViteResolve,
  #[napi(value = "builtin:wasm-fallback")]
  WasmFallback,
  #[napi(value = "builtin:wasm-helper")]
  WasmHelper,
  #[napi(value = "builtin:web-worker-post")]
  WebWorkerPost,
}
