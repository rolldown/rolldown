use napi_derive::napi;

#[derive(Debug)]
#[napi(string_enum)]
pub enum BindingBuiltinPluginName {
  #[napi(value = "builtin:esm-external-require")]
  EsmExternalRequire,
  #[napi(value = "builtin:isolated-declaration")]
  IsolatedDeclaration,
  #[napi(value = "builtin:module-preload-polyfill")]
  ModulePreloadPolyfill,
  #[napi(value = "builtin:react-refresh-wrapper")]
  ReactRefreshWrapper,
  #[napi(value = "builtin:reporter")]
  Report,
  #[napi(value = "builtin:replace")]
  Replace,
  #[napi(value = "builtin:transform")]
  Transform,
  #[napi(value = "builtin:vite-alias")]
  ViteAlias,
  #[napi(value = "builtin:vite-asset")]
  ViteAsset,
  #[napi(value = "builtin:vite-asset-import-meta-url")]
  ViteAssetImportMetaUrl,
  #[napi(value = "builtin:vite-build-import-analysis")]
  ViteBuildImportAnalysis,
  #[napi(value = "builtin:vite-css")]
  ViteCSS,
  #[napi(value = "builtin:vite-css-post")]
  ViteCSSPost,
  #[napi(value = "builtin:vite-dynamic-import-vars")]
  ViteDynamicImportVars,
  #[napi(value = "builtin:vite-html")]
  ViteHtml,
  #[napi(value = "builtin:vite-html-inline-proxy")]
  ViteHtmlInlineProxy,
  #[napi(value = "builtin:vite-import-glob")]
  ViteImportGlob,
  #[napi(value = "builtin:vite-json")]
  ViteJson,
  #[napi(value = "builtin:vite-load-fallback")]
  ViteLoadFallback,
  #[napi(value = "builtin:vite-manifest")]
  ViteManifest,
  #[napi(value = "builtin:vite-resolve")]
  ViteResolve,
  #[napi(value = "builtin:wasm-fallback")]
  WasmFallback,
  #[napi(value = "builtin:wasm-helper")]
  WasmHelper,
  #[napi(value = "builtin:web-worker-post")]
  WebWorkerPost,
}
