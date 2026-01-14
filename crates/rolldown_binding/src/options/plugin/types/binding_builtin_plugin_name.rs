use napi_derive::napi;

#[derive(Debug)]
#[napi(string_enum)]
pub enum BindingBuiltinPluginName {
  #[napi(value = "builtin:esm-external-require")]
  EsmExternalRequire,
  #[napi(value = "builtin:isolated-declaration")]
  IsolatedDeclaration,
  #[napi(value = "builtin:replace")]
  Replace,
  #[napi(value = "builtin:vite-alias")]
  ViteAlias,
  #[napi(value = "builtin:vite-build-import-analysis")]
  ViteBuildImportAnalysis,
  #[napi(value = "builtin:vite-dynamic-import-vars")]
  ViteDynamicImportVars,
  #[napi(value = "builtin:vite-import-glob")]
  ViteImportGlob,
  #[napi(value = "builtin:vite-json")]
  ViteJson,
  #[napi(value = "builtin:vite-load-fallback")]
  ViteLoadFallback,
  #[napi(value = "builtin:vite-manifest")]
  ViteManifest,
  #[napi(value = "builtin:vite-module-preload-polyfill")]
  ViteModulePreloadPolyfill,
  #[napi(value = "builtin:vite-react-refresh-wrapper")]
  ViteReactRefreshWrapper,
  #[napi(value = "builtin:vite-reporter")]
  ViteReporter,
  #[napi(value = "builtin:vite-resolve")]
  ViteResolve,
  #[napi(value = "builtin:vite-transform")]
  ViteTransform,
  #[napi(value = "builtin:vite-wasm-fallback")]
  ViteWasmFallback,
  #[napi(value = "builtin:vite-wasm-helper")]
  ViteWasmHelper,
  #[napi(value = "builtin:vite-web-worker-post")]
  ViteWebWorkerPost,
}
