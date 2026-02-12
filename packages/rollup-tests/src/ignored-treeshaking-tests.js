const ignoreTests = [
  // The treeshaking is not compatible with rollup
  "rollup@form@for-in-scopes: properly associate or shadow variables in and around for-in-loops", // the treeshaking affect deconfilct
  "rollup@form@curried-function: properly handles a curried function", // the treeshaking affect deconfilct
  "rollup@form@effect-in-for-of-loop: includes effects in for-of loop (#870)@generates es",// `const x = xxx`, x unused
  "rollup@form@for-loop-assignment: removes assignments with computed indexes in for loops",
  "rollup@form@for-of-scopes: properly associate or shadow variables in and around for-of-loops",
  "rollup@form@for-scopes: properly associate or shadow variables in and around for-loops@generates es",
  "rollup@form@function-mutation: function-mutations do not have effects@generates es",
  "rollup@form@getter-return-values: forwards return values of getters",
  "rollup@form@super-classes@super-class-prototype-access: correctly resolves the prototype of the super class when accessing properties",
  "rollup@form@import-named-exported-global-with-alias: allow globals to be exported and imported",
  "rollup@form@literals-from-class-statics: tracks literal values in class static fields", // minify feature
  "rollup@form@logical-expression@mutate-logical-expression: properly handle the results of mutating logical expressions@generates es",
  "rollup@form@namespace-missing-export-effects: handles interacting with missing namespace members", // the cross module const folding
  "rollup@form@namespace-optimization-computed-string: it does dynamic lookup optimization of internal namespaces for string-literal keys@generates es",
  "rollup@form@nested-this-expressions: properly keep or ignore nested \"this\"-expressions",
  "rollup@form@object-expression@proto-property: Deoptimize when __proto__ is used", // minify feature
  "rollup@form@property-setters-and-getters@early-access-getter-return: handles accessing the return expression of a getter before it has been bound",
  "rollup@form@property-setters-and-getters@early-access-getter-value: handles accessing the value of a getter before it has been bound",
  "rollup@form@property-setters-and-getters@shadowed-setters: handles setters shadowed by computed setters",
  "rollup@form@prototype-functions: properly includes prototype functions",// const folding
  "rollup@form@redeclarations: make sure re-declarations via var and function are linked properly",
  "rollup@form@render-removed-statements: make sure removed statements do no leave unwanted white-space",
  "rollup@form@simplify-return-expression: Simplifies conditionals in return expression",
  "rollup@form@switch-scopes: correctly handles switch scopes",
  "rollup@form@this-in-imports: properly keep or ignore \"this\"-expressions when calling imported functions",
  "rollup@form@unmodified-default-exports: does not treat property assignment as rebinding for sake of unbound default exports",
  "rollup@form@try-statement-deoptimization@supports-core-js: supports core-js feature detection (#2869)",
  "rollup@form@pure-comments-disabled: does not rely on pure annotations if they are disabled",
  "rollup@form@tdz-access-in-declaration: detect accessing TDZ variables within the declaration",
  "rollup@function@tree-shake-variable-declarations-2: remove unused variables from declarations (#1831)",
  "rollup@function@respect-default-export-reexporter-side-effects: respect side-effects in reexporting modules even if moduleSideEffects are off",
  "rollup@function@respect-reexporter-side-effects: respect side-effects in reexporting modules even if moduleSideEffects are off",
  "rollup@function@namespace-member-side-effects@assignment: checks side effects when reassigning namespace members",
  "rollup@form@mutations-in-imports: track mutations of imports",
  "rollup@form@destructured-known-arguments: tracks known argument values through destructuring",
  "rollup@form@logical-expression-with-property-access: keep logical expressions",
  "rollup@form@function-scopes: properly associate or shadow variables in and around functions",
  "rollup@form@object-expression-treeshaking@only-include-destructured-parameter-props: only includes destructured parameter props",
  "rollup@form@object-expression-treeshaking@track-through-chain-expressions: tracks property access through optional chains",
  "rollup@form@optional-chaining-missing-properties: supports optional chaining for missing properties",
  "rollup@form@recursive-values: do not fail for pathological recursive algorithms and circular structures",
  "rollup@function@class-static-block-tree-shaking: treeshakes no effect static blocks",
  "rollup@function@tree-shaking-proxy-2: Make tree-shaking work on the handler of the Proxy",
  "rollup@form@namespace-tostring@inlined-namespace-static-resolution: statically resolves Symbol.toStringTag for inlined namespaces",
  "rollup@form@object-expression-treeshaking@preserve-unknown-wellknown: always preserves well-known properties from objects if Rollup does not know about them",
  "rollup@form@using-statement-symbol-usage: preserves Symbol.dispose side effects when used in using statement",
  "rollup@form@namespace-mutation: does not automatically include the entire namespace if members are mutated"
]

// Generated by packages/rollup-tests/test/form/found-tree-shaking-not-align.js
const ignoredTreeshakingTests = require('./ignored-treeshaking-tests.json')

module.exports = ignoreTests.concat(ignoredTreeshakingTests)
