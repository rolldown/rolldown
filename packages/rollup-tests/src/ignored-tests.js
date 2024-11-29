// cSpell:disable
const ignoreTests = [
  'rollup@function@bundle-facade-order: respects the order of entry points when there are additional facades for chunks', // https://github.com/rolldown/rolldown/issues/1842#issuecomment-2296345255

  // The test case import test.js from rollup package, it's dependencies can't be resolved.
  "rollup@function@relative-outside-external: correctly resolves relative external imports from outside directories",
  // Ignore skipIfWindows test avoid test status error
  'rollup@function@preserve-symlink: follows symlinks',
  'rollup@function@symlink: follows symlinks',
  "rollup@form@sourcemaps-inline: correct sourcemaps are written (inline)@generates es",

  // The `RenderChunk#modules` should ignores non-bundled modules
  "rollup@function@inline-dynamic-imports-bundle: ignores non-bundled modules when inlining dynamic imports",
 
  // The result is not working as expected
  "rollup@function@module-side-effect-reexport: includes side effects of re-exporters unless they have moduleSideEffects: false",// https://github.com/rolldown/rolldown/issues/2864
  "rollup@form@hoisted-vars-in-dead-branches: renders hoisted variables in dead branches", // https://github.com/oxc-project/oxc/issues/7209
  "rollup@function@hoisted-variable-if-else: handles hoisted variables in chained if statements",// https://github.com/oxc-project/oxc/issues/7209
  "rollup@function@argument-deoptimization@global-calls: tracks argument mutations of calls to globals", // need as esm if module is unknow-format and add `use strcit` to the output, https://github.com/rolldown/rolldown/issues/2394

  // /*@__PURE__*/ related
  "rollup@form@pure-comment-scenarios-complex: correctly handles various advanced pure comment scenarios",// https://github.com/oxc-project/oxc/issues/7501 https://github.com/oxc-project/oxc/issues/7209#issuecomment-2503133537 The `assigned to unreferenced var will be dropped` is a minify featrue
  "rollup@form@nested-pure-comments: correctly associates pure comments before sequence expressions etc.", // The Sequence expression/Binary expression/Calls with parentheses is not implement

  // deconfilct
  "rollup@function@class-name-conflict-2: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict-3: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict-4: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict: preserves class names even if the class is renamed",
  "rollup@form@assignment-to-exports-class-declaration: does not rewrite class expression IDs@generates es",

  // watch mode api is not compitable with rollup, the `rollup.watch` is not a promise at rollup
  "rollup@hooks@Throws when not specifying \"file\" or \"dir\"",
  "rollup@hooks@Throws when using the \"file\"\" option for multiple chunks",
  "rollup@hooks@Throws when using the \"sourcemapFile\" option for multiple chunks",
  "rollup@hooks@allows to enforce plugin hook order in watch mode",
  "rollup@hooks@allows to enforce sequential plugin hook order in watch mode",

  "rollup@hooks@keeps emitted ids stable between runs",// emit empty asset
  "rollup@hooks@passes errors to the buildEnd hook", // should call buildEnd hook if error
  "rollup@hooks@supports renderError hook", // should call renderError hook if error
  "rollup@hooks@assigns chunk IDs before creating outputBundle chunks", // The `renderChunk` is called at parallel, collect chunk info to array is unstable.  https://github.com/rolldown/rolldown/issues/2364
]

module.exports = {
  ignoreTests
}
