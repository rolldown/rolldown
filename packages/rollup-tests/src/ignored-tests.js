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
]

module.exports = {
  ignoreTests,
}
