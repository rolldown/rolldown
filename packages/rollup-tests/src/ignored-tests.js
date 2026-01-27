const ignoreTests = [
  // # Tests ported to other locations
  // ## These tests are moved to package/rolldown/tests
  // https://github.com/rolldown/rolldown/pull/5715
  "rollup@hooks@passes errors from closeBundle hook",
  "rollup@hooks@supports closeBundle hook",

  // ## These tests are moved to crates/rolldown/tests/rollup
  "rollup@form@jsx@preserves-jsx-attributes: preserves JSX with string attributes output",
  "rollup@form@jsx@preserves-jsx-child: preserves JSX children",
  "rollup@form@jsx@preserves-jsx-closing: preserves JSX closing element",
  "rollup@form@jsx@preserves-jsx-empty-expression: preserves JSX output",
  "rollup@form@jsx@preserves-jsx-expression: preserves JSX expressions",
  "rollup@form@jsx@preserves-jsx-fragment: preserves JSX output",
  "rollup@form@jsx@preserves-jsx-member-expression: preserves JSX member expressions",
  "rollup@form@jsx@preserves-jsx-self-closing: preserves self-closing JSX elements",
  "rollup@form@jsx@preserves-jsx-spread-attribute: preserves JSX spread attributes",
  "rollup@form@jsx@preserves-jsx-spread-child: preserves JSX spread children",
  "rollup@form@jsx@preserves-jsx-text: preserves JSX text",
  "rollup@form@jsx@preserves-native-elements: preserves native JSX elements",
  "rollup@form@jsx@preserves-react: preserves React variable when preserving JSX output",
  "rollup@form@jsx@preserves-react-global: preserves React variable when preserving JSX output",
  "rollup@form@jsx@preserves-react-internal: preserves internal React variable when preserving JSX output",
  "rollup@form@jsx@react-jsx-declarations-with-key-attribute: JSX with react-jsx uses correct semicolon positions in variable declarations with key attributes",
  "rollup@form@jsx@transpiles-automatic-with-defaults: transpiles JSX for react",
  "rollup@form@jsx@transpiles-classic-with-defaults: transpiles JSX for react",
  "rollup@form@jsx@transpiles-empty-fragment: transpiles JSX for react",
  "rollup@form@jsx@transpiles-jsx-attributes: transpiles JSX with string attributes output",
  "rollup@form@jsx@transpiles-jsx-child: transpiles JSX children",
  "rollup@form@jsx@transpiles-jsx-closing: transpiles JSX closing element",
  "rollup@form@jsx@transpiles-jsx-empty-expression: transpiles JSX output",
  "rollup@form@jsx@transpiles-jsx-expression: transpiles JSX expressions",
  "rollup@form@jsx@transpiles-jsx-fragment: transpiles JSX output",
  "rollup@form@jsx@transpiles-jsx-member-expression: transpiles JSX member expressions",
  "rollup@form@jsx@transpiles-jsx-self-closing: transpiles self-closing JSX elements",
  "rollup@form@jsx@transpiles-jsx-spread-attribute: transpiles JSX spread attributes",
  "rollup@form@jsx@transpiles-jsx-spread-child: transpiles JSX spread children",
  "rollup@form@jsx@transpiles-jsx-text: transpiles JSX text",
  "rollup@form@jsx@transpiles-native-elements: preserves native JSX elements",
  "rollup@form@jsx@transpiles-react: transpiles JSX for react",
  "rollup@form@jsx@transpiles-react-global: transpiles JSX for react",
  "rollup@form@jsx@transpiles-react-internal: transpiles JSX for react",
  "rollup@form@jsx@transpiles-react-jsx: transpiles JSX for react",
  "rollup@form@jsx@transpiles-react-jsx-expression-semicolon: Adds semicolons at the correct positions in transpiled JSX",
  "rollup@function@jsx@missing-jsx-export: throws when the JSX factory is not exported",

  // --------------------------------------------------------------------------------------
  // # Test infraructure related ignores
  // ## Ignore skipIfWindows test avoid test status error
  'rollup@function@preserve-symlink: follows symlinks',
  'rollup@function@symlink: follows symlinks',
  "rollup@form@sourcemaps-inline: correct sourcemaps are written (inline)@generates es",

  // --------------------------------------------------------------------------------------
  // # Expected behavior differences
  // ## build starts when `bundle.generate` / `bundle.write` is called instead of `rollup.rollup`
  "rollup@hooks@supports buildStart and buildEnd hooks",
  "rollup@hooks@supports warnings in buildStart and buildEnd hooks",
  "rollup@hooks@passes errors to the buildEnd hook",

  // ## import.meta.url polyfill behaves differently
  "rollup@function@import-meta-url-b: Access document.currentScript at the top level",
  "rollup@form@import-meta-url: supports import.meta.url@generates es",
  "rollup@form@resolve-import-meta-url-export: correctly exports resolved import.meta.url@generates es",
  "rollup@form@resolve-import-meta-url: allows to configure import.meta.url@generates es",
  "rollup@function@import-meta-url-with-compact: Get the right URL with compact output",

  // ## Rollup treats non-js-extensions module as js module, but Rolldown will guess the module type from the externsion
  "rollup@function@non-js-extensions: non .js extensions are preserved",
  "rollup@function@error-parse-unknown-extension: throws with an extended error message when failing to parse a file without .(m)js extension",
  "rollup@function@error-parse-json: throws with an extended error message when failing to parse a file with \".json\" extension",

  // ## warning / error differences
  "rollup@function@plugin-hook-filters: plugin hook filter is supported", // Rolldown has additional `EMPTY_IMPORT_META` warning
  "rollup@function@generate-bundle-mutation: handles adding or deleting symbols in generateBundle", // rolldown outputs a warning when assigning to bundle[foo]
  "rollup@function@missing-entry-export: throws when exporting something that does not exist from an entry", // rolldown uses `PARSE_ERROR` instead of `MISSING_EXPORT`
  // ### error message difference
  "rollup@hooks@Throws when using the \"file\"\" option for multiple chunks",
  "rollup@function@logging@plugin-order: allows to order plugins when logging",
  "rollup@function@logging@log-from-plugin-onlog-onwarn: passes logs from plugins to onLog and onwarn",
  "rollup@function@logging@log-from-plugin-onlog: passes logs from plugins to onLog",
  "rollup@function@logging@log-from-plugin-onwarn: passes warn logs from plugins to onwarn",
  "rollup@function@logging@log-from-plugin-options-onlog-onwarn: passes logs from plugins to onLog and onwarn",
  "rollup@function@logging@log-from-plugin-options-onlog: passes logs from plugins to onLog",
  "rollup@function@logging@log-from-plugin-options-onwarn: passes warn logs from plugins to onwarn",
  "rollup@function@logging@log-from-plugin-simple: prints logs from plugins via input options if there are no handlers",
  "rollup@function@logging@loglevel-debug: shows all logs for logLevel:debug",
  "rollup@function@logging@loglevel-info: does not show debug logs for logLevel:info",
  "rollup@function@logging@loglevel-warn: only shows warning logs for logLevel:warn",
  "rollup@function@cannot-call-external-namespace: warns if code calls an external namespace",
  "rollup@function@cannot-call-internal-namespace: warns if code calls an internal namespace",
  // ### rolldown uses `ASSIGN_TO_IMPORT` while rollup uses `ILLEGAL_REASSIGNMENT`
  "rollup@function@ast-validations@reassign-import-fails: disallows assignments to imported bindings",
  "rollup@function@ast-validations@reassign-import-not-at-top-level-fails: disallows assignments to imported bindings not at the top level",
  "rollup@function@ast-validations@update-expression-of-import-fails: disallows updates to imported bindings",
  // ### assignment to a const variable is an error instead of a warning in Rolldown
  "rollup@function@warning-const-reassign: Cannot reassign a variable declared with `const`",
  "rollup@function@namespace-reassign-import-fails: warns for reassignments to namespace exports",
  "rollup@function@namespace-update-import-fails: disallows updates to namespace exports",

  // ## tests relying on plugins
  "rollup@function@no-treeshake-react: passes when bundling React without tree-shaking", // relies on @rollup/plugin-node-resolve
  "rollup@function@strip-bom-1: Works correctly with BOM files and the @rollup/plugin-commonjs plugin.", // relies on @rollup/plugin-commonjs
  "rollup@form@supports-core-js: supports core-js", // relies on @rollup/plugin-commonjs
  "rollup@form@supports-es5-shim: supports es5-shim", // relies on @rollup/plugin-commonjs
  "rollup@form@supports-es6-shim: supports es6-shim", // relies on @rollup/plugin-commonjs

  // ## Order not guaranteed due to parallelism
  "rollup@hooks@assigns chunk IDs before creating outputBundle chunks", // The `renderChunk` is called at parallel, collect chunk info to array is unstable.  https://github.com/rolldown/rolldown/issues/2364
  "rollup@function@external-resolved: passes both unresolved and resolved ids to the external option", // Rolldown runs in parallel. The order can be different.

  // ## `external: true` is not supported but it's not documented
  "rollup@function@external-namespace-and-default-reexport-compat2: reexports both a namespace, the namespace as a name and the default export when using compat interop",
  "rollup@function@external-namespace-and-default-reexport-compat3: reexports both a namespace and the default export when using compat interop",
  "rollup@function@external-namespace-and-default-reexport-compat: reexports both a namespace and the default export when using compat interop",

  // ## Others
  'rollup@hooks@Throws when not specifying "file" or "dir"', // Rolldown defaults `output.dir` to `dist`
  'rollup@function@bundle-facade-order: respects the order of entry points when there are additional facades for chunks', // https://github.com/rolldown/rolldown/issues/1842#issuecomment-2296345255
  "rollup@function@argument-deoptimization@global-calls: tracks argument mutations of calls to globals", // need as esm if module is unknown-format and add `use strcit` to the output, https://github.com/rolldown/rolldown/issues/2394
  "rollup@function@es5-class-called-without-new: does not swallow type errors when running constructor functions without \"new\"", // rolldown align directive rendering with esbuild
  "rollup@function@jsx@unknown-mode: throws when using an unknown jsx mode", // validation is done before test are run
  "rollup@function@jsx@unnecessary-import-source: throws when preserving JSX syntax with an unnecessary import source", // `jsx.importSource` cannot be set with `jsx: 'preserve'`
  "rollup@function@catch-rust-panic: Catch Rust panics and then throw them in Node", // specific to Rollup's implementation
  "rollup@function@exports-are-not-defined: Throw descriptive error message for used export is not defined", // the input code triggers a different error in rolldown
  "rollup@function@dynamic-import-call-method-with-this-await: includes the correct \"this\" context when calling a method on a dynamically imported module via \"await\"" // Rolldown does not necessarily keep the `this` value for exported functions
]

module.exports = {
  ignoreTests
}
