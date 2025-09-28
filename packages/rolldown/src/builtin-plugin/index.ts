// Core exports
export { BuiltinPlugin } from './constructor';
export { bindingifyBuiltInPlugin } from './utils';

// Common plugins
export {
  assetPlugin,
  buildImportAnalysisPlugin,
  dynamicImportVarsPlugin,
  esmExternalRequirePlugin,
  importGlobPlugin,
  isolatedDeclarationPlugin,
  jsonPlugin,
  loadFallbackPlugin,
  manifestPlugin,
  modulePreloadPolyfillPlugin,
  reactRefreshWrapperPlugin,
  reporterPlugin,
  viteResolvePlugin,
  wasmFallbackPlugin,
  wasmHelperPlugin,
  webWorkerPostPlugin,
} from './plugins';

// Alias plugin
export { aliasPlugin } from './alias-plugin';

// Replace plugin
export { replacePlugin } from './replace-plugin';

// Transform plugin
export { transformPlugin } from './transform-plugin';
