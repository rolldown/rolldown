// cSpell:disable
const ignoreTests = [
  'rollup@function@bundle-facade-order: respects the order of entry points when there are additional facades for chunks', // https://github.com/rolldown/rolldown/issues/1842#issuecomment-2296345255

  // Ignore skipIfWindows test avoid test status error
  'rollup@function@preserve-symlink: follows symlinks',
  'rollup@function@symlink: follows symlinks',
  "rollup@form@sourcemaps-inline: correct sourcemaps are written (inline)@generates es",
 
  // The result is not working as expected
  "rollup@function@module-side-effect-reexport: includes side effects of re-exporters unless they have moduleSideEffects: false",// https://github.com/rolldown/rolldown/issues/2864
  "rollup@form@hoisted-vars-in-dead-branches: renders hoisted variables in dead branches", // https://github.com/oxc-project/oxc/issues/7209
  "rollup@function@hoisted-variable-if-else: handles hoisted variables in chained if statements",// https://github.com/oxc-project/oxc/issues/7209
  "rollup@function@argument-deoptimization@global-calls: tracks argument mutations of calls to globals", // need as esm if module is unknow-format and add `use strcit` to the output, https://github.com/rolldown/rolldown/issues/2394

  // /*@__PURE__*/ related
  "rollup@form@pure-comment-scenarios-complex: correctly handles various advanced pure comment scenarios",// https://github.com/oxc-project/oxc/issues/7501 https://github.com/oxc-project/oxc/issues/7209#issuecomment-2503133537 The `assigned to unreferenced var will be dropped` is a minify featrue
  "rollup@form@nested-pure-comments: correctly associates pure comments before sequence expressions etc.", // The Sequence expression/Binary expression/Calls with parentheses is not implement

  "rollup@hooks@assigns chunk IDs before creating outputBundle chunks", // The `renderChunk` is called at parallel, collect chunk info to array is unstable.  https://github.com/rolldown/rolldown/issues/2364
  "rollup@form@non-empty-block-statement: do not remove non an empty block statement@generates es", // https://github.com/rolldown/rolldown/pull/3541#issuecomment-2649731213
  "rollup@function@es5-class-called-without-new" // rolldown align directive rendering with esbuild
]

module.exports = {
  ignoreTests
}
