// cSpell:disable
const ignoreTests = [

  // --- following tests will hang forever ---

  // FATAL ERROR: threadsafe_function.rs:573
  'rollup@function@external-ignore-reserved-null-marker: external function ignores \\0 started ids',

  // Need to investigate
  'rollup@function@bundle-facade-order: respects the order of entry points when there are additional facades for chunks',

  // The test case import test.js from rollup package, it's dependencies can't be resolved.
  "rollup@function@relative-outside-external: correctly resolves relative external imports from outside directories",
  // Ignore skipIfWindows test avoid test status error
  'rollup@function@preserve-symlink: follows symlinks',
  'rollup@function@symlink: follows symlinks',
  "rollup@form@sourcemaps-inline: correct sourcemaps are written (inline)@generates es",

  // The rolldown output chunk including `module comment` caused line offset, the rollup provider the fake sourcemap can't remapping.
  "rollup@sourcemaps@render-chunk-babili: generates valid sourcemap when source could not be determined@generates es",
  // Here has unexpected error `Error: nul byte found in provided data at position: 0` from rust due to #967.
  // It crashed at call `banner` function at rust. 
  "rollup@sourcemaps@excludes-plugin-helpers: excludes plugin helpers from sources@generates es",

  // The output plugins is not working
  "rollup@form@per-output-plugins: allows specifying per-output plugins@generates es",

  // The `RenderChunk#modules` should ignores non-bundled modules
  "rollup@function@inline-dynamic-imports-bundle: ignores non-bundled modules when inlining dynamic imports",
 
  // The result is not working as expected
  "rollup@function@module-side-effect-reexport: includes side effects of re-exporters unless they have moduleSideEffects: false",// https://github.com/rolldown/rolldown/issues/2864
  "rollup@form@hoisted-vars-in-dead-branches: renders hoisted variables in dead branches", // https://github.com/oxc-project/oxc/issues/7209
  "rollup@function@hoisted-variable-if-else: handles hoisted variables in chained if statements",// https://github.com/oxc-project/oxc/issues/7209
  "rollup@form@mutations-in-imports: track mutations of imports",
 
  // /*@__PURE__*/ related
  "rollup@form@pure-comment-scenarios-complex: correctly handles various advanced pure comment scenarios",
  "rollup@form@nested-pure-comments: correctly associates pure comments before sequence expressions etc.", 

  // deconfilct
  "rollup@function@class-name-conflict-2: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict-3: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict-4: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict: preserves class names even if the class is renamed",
  "rollup@form@assignment-to-exports-class-declaration: does not rewrite class expression IDs@generates es",

  // comment related
  "rollup@form@comment-before-import: preserves comments before imports@generates es",
  "rollup@form@comment-start-inside-comment: properly remove coments above import statements@generates es",

  // The output plugins hooks is not working as expected
  "rollup@function@options-in-renderstart: makes input and output options available in renderStart",

  // Nested plugin is not supported
  "rollup@function@nested-and-async-plugin: works when nested plugin",

  // The output code/sourcemap is not same as rollup,
  "rollup@function@sourcemap-true-generatebundle: emits sourcemaps before generateBundle hook",
  "rollup@function@sourcemap-inline-generatebundle: includes inline sourcemap comments in generateBundle hook",

  // import.meta.ROLLUP_FILE_URL_<referenceId> is not supported
  "rollup@form@emit-asset-file: supports emitting assets from plugin hooks@generates es",
  "rollup@form@emit-uint8array-no-buffer: supports emitting assets as Uint8Arrays when Buffer is not available@generates es",

  // Module meta related
  // Shouldn't modify meta objects passed in resolveId hook
  "rollup@function@custom-module-options: supports adding custom options to modules",

  // Should delete use strict from function body
  "rollup@function@function-use-strict-directive-removed: should delete use strict from function body",

  // The sourcemap related
  "rollup@function@handles-stringified-sourcemaps: handles transforms that return stringified source maps (#377)",
  "rollup@function@transform-without-sourcemap-render-chunk: preserves sourcemap chains when transforming",
  "rollup@sourcemaps@basic-support: basic sourcemap support@generates es",
  "rollup@sourcemaps@names: names are recovered (https://github.com/rollup/rollup/issues/101)@generates es",
  "rollup@sourcemaps@single-length-segments: handles single-length sourcemap segments@generates es",
  "rollup@sourcemaps@transform-low-resolution: handles combining low-resolution and high-resolution source-maps when transforming@generates es",
  "rollup@form@render-chunk-plugin-sourcemaps: supports returning undefined source maps from render chunk hooks, when source maps are enabled@generates es", // file not expected
]

module.exports = {
  ignoreTests
}
