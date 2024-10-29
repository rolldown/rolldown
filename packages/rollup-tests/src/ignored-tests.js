// cSpell:disable
const ignoreTests = [
  // The giving code is not valid JavaScript.
  'rollup@function@circular-default-exports: handles circular default exports',
  // Panic: TODO: supports
  'rollup@function@dynamic-import-rewriting: Dynamic import string specifier resolving',
  'rollup@function@deprecated@dynamic-import-name-warn: warns when specifying a custom importer function for formats other than "es"',

  // --- following tests will hang forever ---

  // Import Assertions related
  'rollup@function@import-assertions@plugin-assertions-this-resolve: allows plugins to provide assertions for this.resolve',
"rollup@function@import-assertions@plugin-assertions-this-resolve: allows plugins to provide attributes for this.resolve",
  "rollup@function@import-assertions@warn-assertion-conflicts: warns for conflicting import attributes",
  "rollup@function@import-assertions@warn-unresolvable-assertions: warns for dynamic import attributes that cannot be resolved",
  // FATAL ERROR: threadsafe_function.rs:573
  'rollup@function@external-ignore-reserved-null-marker: external function ignores \\0 started ids',

  // Need to investigate
  'rollup@function@bundle-facade-order: respects the order of entry points when there are additional facades for chunks',

  // Not supported
  'rollup@function@enforce-plugin-order: allows to enforce plugin hook order',

  // blocked by supporting `output.preserveModules: true`
  'rollup@function@preserve-modules-default-mode-namespace: import namespace from chunks with default export mode when preserving modules',
  // The test case import test.js from rollup package, it's dependencies can't be resolved.
  "rollup@function@relative-outside-external: correctly resolves relative external imports from outside directories",
  // Ignore skipIfWindows test avoid test status error
  'rollup@function@preserve-symlink: follows symlinks',
  'rollup@function@symlink: follows symlinks',
  // The rolldown output chunk including `module comment` caused line offset, the rollup provider the fake sourcemap can't remapping.
  "rollup@sourcemaps@render-chunk-babili: generates valid sourcemap when source could not be determined@generates es",
  // Here has unexpected error `Error: nul byte found in provided data at position: 0` from rust due to #967.
  // It crashed at call `banner` function at rust. 
  "rollup@sourcemaps@excludes-plugin-helpers: excludes plugin helpers from sources@generates es",

  // output.sourcemapBaseUrl is not supported
  "rollup@function@sourcemap-base-url-invalid: throws for invalid sourcemapBaseUrl",
  "rollup@sourcemaps@sourcemap-base-url-without-trailing-slash: add a trailing slash automatically if it is missing@generates es",
  "rollup@sourcemaps@sourcemap-base-url: adds a sourcemap base url@generates es",
  // PluginContext.getCombinedSourcemap is not supported
  "rollup@sourcemaps@combined-sourcemap-with-loader: get combined sourcemap in transforming with loader@generates es",
  "rollup@sourcemaps@combined-sourcemap: get combined sourcemap in transforming@generates es",
  // The output code/sourcemap is not same as rollup,
  "rollup@function@sourcemap-true-generatebundle: emits sourcemaps before generateBundle hook",
  "rollup@function@sourcemap-inline-generatebundle: includes inline sourcemap comments in generateBundle hook",
  // invalid output.exports should not panic
  "rollup@function@export-type-mismatch-b: export type must be auto, default, named or none",
  // format amd not supported
  "rollup@function@amd-auto-id-id: throws when using both the amd.autoId and the amd.id option",
  "rollup@function@amd-base-path-id: throws when using both the amd.basePath and the amd.id option",
  "rollup@function@amd-base-path: throws when using only amd.basePath option",
  // The input option is emtpy string
  "rollup@function@avoid-variable-be-empty: avoid variable from empty module name be empty",
  // output.preserveModules is not supported
  "rollup@function@circular-preserve-modules: correctly handles circular dependencies when preserving modules",
  "rollup@function@missing-export-preserve-modules: supports shimming missing exports when preserving modules",
  "rollup@function@preserve-modules-circular-order: preserves execution order for circular dependencies when preserving modules",
  "rollup@function@preserve-modules@inline-dynamic-imports: Inlining dynamic imports is not supported when preserving modules",
  "rollup@function@preserve-modules@invalid-default-export-mode: throws when using default export mode with named exports",
  "rollup@function@preserve-modules@invalid-no-preserve-entry-signatures: throws when setting preserveEntrySignatures to false",
  "rollup@function@preserve-modules@invalid-none-export-mode: throws when using none export mode with named exports",
  "rollup@function@preserve-modules@manual-chunks: Assigning manual chunks fails when preserving modules",
  "rollup@function@preserve-modules@mixed-exports: warns for mixed exports in all chunks when preserving modules",
  "rollup@function@preserve-modules@virtual-modules-conflict: Generates actual files for virtual modules when preserving modules",
  "rollup@function@preserve-modules@virtual-modules: Generates actual files for virtual modules when preserving modules",
  "rollup@function@synthetic-named-exports@preserve-modules: handles a dynamic import with synthetic named exports in preserveModules mode",
  "rollup@function@circular-namespace-reexport-preserve-modules: correctly handles namespace reexports with circular dependencies when preserving modules",
  // output.manualChunks is not supported
  "rollup@function@manual-chunks-conflict: Throws for conflicts between manual chunks",
  "rollup@function@manual-chunks-include-external-modules3: throws an error EXTERNAL_MODULES_CANNOT_BE_TRANSFORMED_TO_MODULES for manualChunks' modules that are resolved as an external module by the 'external' option",
  "rollup@function@manual-chunks-include-external-modules: throws for manualChunks' modules that are resolved as an external module by plugins",
  "rollup@function@manual-chunks-info: provides additional chunk information to a manualChunks function",
  "rollup@function@circular-namespace-reexport-manual-chunks: correctly handles namespace reexports with circular dependencies when using manual chunks",
  "rollup@function@emit-chunk-manual-asset-source: supports setting asset sources as side effect of the manual chunks option",
  "rollup@function@emit-chunk-manual: supports emitting chunks as side effect of the manual chunks option",
  "rollup@function@inline-imports-with-manual: Manual chunks are not supported when inlining dynamic imports",
  // PluginContext.setAssetSource is not supported
  // Should throw error if asset source is null
  "rollup@function@emit-file@asset-source-invalid2: throws when setting an empty asset source",
  "rollup@function@emit-file@asset-source-invalid3: throws when setting an empty asset source",
  "rollup@function@emit-file@asset-source-invalid4: throws when setting an empty asset source",
  // Should throw error if PluginContext.emitFile asset source is null
  "rollup@function@emit-file@asset-source-invalid: throws when setting an empty asset source",
  // PluginContext.getFilename throw error if asset source is not set
  "rollup@function@emit-file@asset-source-missing3: throws when accessing the file name before the asset source is set",
  "rollup@function@emit-file@asset-source-missing4: throws when accessing the file name before the asset source is set",
  // Should throw error if asset source is not set at generate stage
  "rollup@function@emit-file@asset-source-missing2: throws when not setting the asset source",
  "rollup@function@emit-file@asset-source-missing5: throws when not setting the asset source and accessing the asset URL",
  // import.meta.ROLLUP_FILE_URL_<referenceId> throw error if asset source is not set
  "rollup@function@emit-file@asset-source-missing: throws when not setting the asset source",
  // PluginContext.emitFile is not supported emit chunk
  "rollup@function@emit-chunk-hash: gives access to the hashed filed name via this.getFileName in generateBundle",
  // Should throw error if input option key is `./path` or `/path` or `../path`
  "rollup@function@input-name-validation2: throws for relative paths as input names",
  "rollup@function@input-name-validation3: throws for relative paths as input names",
  "rollup@function@input-name-validation: throws for absolute paths as input names",
  // syntheticNamedExports is not supported
  "rollup@function@synthetic-named-exports-fallback-es2015: adds a fallback in case synthetic named exports are falsy",
  "rollup@function@synthetic-named-exports-fallback: adds a fallback in case synthetic named exports are falsy",
  "rollup@function@synthetic-named-exports@circular-synthetic-exports2: handles circular synthetic exports",
  "rollup@function@synthetic-named-exports@circular-synthetic-exports: handles circular synthetic exports",
  "rollup@function@synthetic-named-exports@dynamic-import: supports dynamically importing a module with synthetic named exports",
  "rollup@function@synthetic-named-exports@entry: does not expose the synthetic namespace if an entry point uses a string value",
  "rollup@function@synthetic-named-exports@external-synthetic-exports: external modules can not have syntheticNamedExports",
  "rollup@function@synthetic-named-exports@namespace-object: does not include named synthetic namespaces in namespace objects",
  "rollup@function@synthetic-named-exports@namespace-overrides: supports re-exported synthetic exports in namespace objects with correct export precedence",
  "rollup@function@synthetic-named-exports@non-default-export: supports providing a named export to generate synthetic exports",
  "rollup@function@synthetic-named-exports@synthetic-exports-need-default: synthetic named exports modules need a default export",
  "rollup@function@synthetic-named-exports@synthetic-exports-need-fallback-export: synthetic named exports modules need their fallback export",
  "rollup@function@synthetic-named-exports@synthetic-named-export-as-default: makes sure default exports of synthetic named exports are snapshots",
  "rollup@function@synthetic-named-exports@synthetic-named-export-entry: does not expose synthetic named exports on entry points",
  // output.generatedCode.symbols is not supported 
  "rollup@function@namespace-tostring@dynamic-import-default-mode: adds Symbol.toStringTag property to dynamic imports of entry chunks with default export mode",
  "rollup@function@namespace-tostring@dynamic-import: adds Symbol.toStringTag property to dynamic imports",
  "rollup@function@namespace-tostring@entry-named: adds Symbol.toStringTag property to entry chunks with named exports",
  "rollup@function@namespace-tostring@external-namespaces: adds Symbol.toStringTag property to external namespaces",
  "rollup@function@namespace-tostring@inlined-namespace: adds Symbol.toStringTag property to inlined namespaces",
  "rollup@function@namespace-tostring@interop-property-descriptor: generated interop namespaces should have correct Symbol.toStringTag",
  "rollup@function@namespace-tostring@property-descriptor: namespace export should have @@toStringTag with correct property descriptors #4336",
]

module.exports = {
  ignoreTests,
}
