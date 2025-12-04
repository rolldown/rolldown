const ignoreTests = [
  // Integrate the test suite into Rolldown
  // https://github.com/rolldown/rolldown/pull/5715
  "rollup@hooks@passes errors from closeBundle hook",
  "rollup@hooks@supports closeBundle hook",

  'rollup@function@bundle-facade-order: respects the order of entry points when there are additional facades for chunks', // https://github.com/rolldown/rolldown/issues/1842#issuecomment-2296345255

  // Ignore skipIfWindows test avoid test status error
  'rollup@function@preserve-symlink: follows symlinks',
  'rollup@function@symlink: follows symlinks',
  "rollup@form@sourcemaps-inline: correct sourcemaps are written (inline)@generates es",

  // The result is not working as expected
  "rollup@function@module-side-effect-reexport: includes side effects of re-exporters unless they have moduleSideEffects: false",// https://github.com/rolldown/rolldown/issues/2864
  "rollup@form@hoisted-vars-in-dead-branches: renders hoisted variables in dead branches", // https://github.com/oxc-project/oxc/issues/7209
  "rollup@function@hoisted-variable-if-else: handles hoisted variables in chained if statements",// https://github.com/oxc-project/oxc/issues/7209
  "rollup@function@argument-deoptimization@global-calls: tracks argument mutations of calls to globals", // need as esm if module is unknown-format and add `use strcit` to the output, https://github.com/rolldown/rolldown/issues/2394

  // /*@__PURE__*/ related
  "rollup@form@pure-comment-scenarios-complex: correctly handles various advanced pure comment scenarios",// https://github.com/oxc-project/oxc/issues/7501 https://github.com/oxc-project/oxc/issues/7209#issuecomment-2503133537 The `assigned to unreferenced var will be dropped` is a minify featrue
  "rollup@form@nested-pure-comments: correctly associates pure comments before sequence expressions etc.", // The Sequence expression/Binary expression/Calls with parentheses is not implement

  "rollup@hooks@assigns chunk IDs before creating outputBundle chunks", // The `renderChunk` is called at parallel, collect chunk info to array is unstable.  https://github.com/rolldown/rolldown/issues/2364
  "rollup@function@external-resolved: passes both unresolved and resolved ids to the external option", // Rolldown runs in parallel. The order can be different.
  "rollup@form@non-empty-block-statement: do not remove non an empty block statement@generates es", // https://github.com/rolldown/rolldown/pull/3541#issuecomment-2649731213
  "rollup@function@es5-class-called-without-new: does not swallow type errors when running constructor functions without \"new\"", // rolldown align directive rendering with esbuild
  "rollup@function@no-treeshake-react: passes when bundling React without tree-shaking", // relies on @rollup/plugin-node-resolve
  "rollup@function@plugin-hook-filters: plugin hook filter is supported", // Rolldown has additional `EMPTY_IMPORT_META` warning
  "rollup@function@strip-bom-1: Works correctly with BOM files and the @rollup/plugin-commonjs plugin.", // relies on @rollup/plugin-commonjs
  "rollup@function@warning-const-reassign: Cannot reassign a variable declared with `const`", // assignment to a const variable is an error instead of a warning in Rolldown

  "rollup@function@generate-bundle-mutation: handles adding or deleting symbols in generateBundle", // rolldown outputs a warning when assigning to bundle[foo]
  "rollup@form@logical-expression@simplify-non-boolean: simplifies logical expressions that resolve statically to non-boolean values", // Oxc DCE is sub-optimal.
  "rollup@form@unary-expressions-preserve-constants: Preserves constant identifiers in unary expressions when constants are used elsewhere", // no need to support

  // `external: true` is not supported but it's not documented
  "rollup@function@external-namespace-and-default-reexport-compat2: reexports both a namespace, the namespace as a name and the default export when using compat interop",
  "rollup@function@external-namespace-and-default-reexport-compat3: reexports both a namespace and the default export when using compat interop",
  "rollup@function@external-namespace-and-default-reexport-compat: reexports both a namespace and the default export when using compat interop",

  // JSX syntax is not supported in JS files
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
  "rollup@form@jsx@preserves-react-global: preserves React variable when preserving JSX output",
  "rollup@form@jsx@preserves-react-internal: preserves internal React variable when preserving JSX output",
  "rollup@form@jsx@preserves-react: preserves React variable when preserving JSX output",
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
  "rollup@form@jsx@transpiles-react-global: transpiles JSX for react",
  "rollup@form@jsx@transpiles-react-internal: transpiles JSX for react",
  "rollup@form@jsx@transpiles-react-jsx: transpiles JSX for react",
  "rollup@form@jsx@transpiles-react: transpiles JSX for react",
  "rollup@function@jsx@missing-jsx-export: throws when the JSX factory is not exported",
  "rollup@function@jsx@unknown-mode: throws when using an unknown jsx mode",
  "rollup@function@jsx@unnecessary-import-source: throws when preserving JSX syntax with an unnecessary import source",
]

module.exports = {
  ignoreTests
}
