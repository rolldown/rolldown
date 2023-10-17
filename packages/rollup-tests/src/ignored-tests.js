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
]

module.exports = {
  ignoreTests,
}
