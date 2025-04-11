// cSpell:disable
module.exports = [
    // Passed, but the output snapshot is different from rollup
    "rollup@form@make-absolute-externals-relative@make-relative-false: does not normalize external paths when set to false",
    "rollup@function@transform-without-code: allows using the transform hook for annotations only without returning a code property and breaking sourcemaps",
    "rollup@form@catch-parameter-shadowing: the parameter of a catch block should correctly shadow an import (#1391)",// rollup not deconflict
    "rollup@form@body-less-for-loops: supports body-less for loops",// rollup not deconflict
    "rollup@form@import-specifier-deshadowing: deshadows aliased import bindings@generates es", // rollup not deconflict
    "rollup@function@transparent-dynamic-inlining: Dynamic import inlining when resolution id is a module in the bundle",
    "rollup@form@dynamic-import-inlining: dynamic import inlining",
    "rollup@form@dynamic-import-inlining-array: supports an array with a single entry when inlining dynamic imports",
    "rollup@form@inline-with-reexport: handles inlining dynamic imports when the imported module contains reexports",
    "rollup@form@nested-inlined-dynamic-import: deconflicts variables when nested dynamic imports are inlined@generates es",
    "rollup@function@duplicate-input-entry: handles duplicate entry modules when using the object form",
    "rollup@form@handles-empty-imports-iife: handles empty imports when generating IIFE output",
    "rollup@form@handles-empty-imports-umd: handles empty imports when generating IIFE output",
    "rollup@form@slash-in-function-parameters: handles slashes in function parameters and correctly inserts missing ids@generates es",
    "rollup@form@render-named-export-declarations: renders named export declarations@generates es",
    "rollup@form@render-declaration-semicolons: properly inserts semi-colons after declarations (#1993)@generates es",
    "rollup@form@removes-existing-sourcemap-comments: removes existing sourcemap comments@generates es",
    "rollup@form@re-export-aliasing: external re-exports aliasing@generates es",
    "rollup@form@pure-class-field: retains pure annotations in class fields",
    "rollup@function@member-expression-assignment-in-function: detect side effect in member expression assignment when not top level",
    "rollup@form@automatic-semicolon-insertion-var: Adds trailing semicolons for modules",
    "rollup@form@base64-deshadow: base64 deshadowing indices",
    "rollup@form@big-int: supports bigint",
    "rollup@form@conflicting-imports: ensures bundle imports are deconflicted (#659)@generates es",
    "rollup@form@deconflict-format-specific-exports: only deconflict \"exports\" for formats where it is necessary@generates es",// avoid unnecessary deconflict
    "rollup@form@deconflict-format-specific-globals: deconflicts format specific globals@generates es",
    "rollup@form@default-export-anonymous-class-extends: handles default exported classes extending a regular expression argument (#4783)",
    "rollup@form@default-export-class: puts the export after the declaration for default exported classes in SystemJS@generates es",
    "rollup@form@default-export-mode: allows specifying the export mode to be \"default\"@generates es",
    "rollup@form@deopt-string-concatenation: deoptimize concatenation when used as an expression statement to better support es5-shim",
    "rollup@form@effect-in-for-of-loop-in-functions: includes effects in for-of loop (#870)@generates es",
    "rollup@form@exponentiation-operator: folds exponentiation operator when considering dead code@generates es",
    "rollup@form@export-default-2: re-exporting a default export@generates es",
    "rollup@form@export-default-3: re-exporting a default export@generates es",
    "rollup@form@export-default-4: single default export in deep namespace@generates es",
    "rollup@form@export-default-anonymous-declarations: export default [Declaration] with spaces and comments@generates es", // avoid rename default function
    "rollup@form@export-default-global: handles default exporting global variables@generates es",
    "rollup@form@export-default-import: correctly exports a default import, even in ES mode (#513)@generates es", // convert reexport to import and export
    "rollup@form@export-default: single (default) exports@generates es",
    "rollup@form@export-internal-namespace-as: supports exporting and resolving internal namespaces as names",
    "rollup@form@export-live-bindings: exported live bindings@generates es",
    "rollup@form@export-namespace-as: supports exporting namespaces as names in entry points@generates es",
    "rollup@form@external-deshadowing: Externals aliases with deshadowing@generates es",
    "rollup@form@external-empty-import-no-global-b: does not expect a global to be provided for empty imports (#1217)@generates es",
    "rollup@form@external-export-tracing: Support external namespace reexport@generates es", // convert reexport to import and export
    "rollup@form@external-import-alias-shadow: handles external aliased named imports that shadow another name@generates es", // avoid deconfilct aliased named imports
    "rollup@form@external-namespace-and-named: Correctly handles external namespace tracing with both namespace and named exports@generates es",
    "rollup@form@external-namespace-reexport: Support external namespace reexport@generates es",
    "rollup@form@for-loop-with-empty-head: handles for loop with empty head@generates es",
    "rollup@form@freeze: supports opt-ing out of usage of Object.freeze@generates es",
    "rollup@form@function-body-return-values: properly extract return values from function bodies",
    "rollup@form@hoisted-variable-case-stmt: Properly handles a variable hoisted from within a fallthrough switch case",
    "rollup@form@import-expression: correctly transforms variables in imported expressions@generates es",
    "rollup@form@import-external-namespace-and-default: disinguishes between external default and namespace (#637)@generates es",
    "rollup@form@internal-conflict-resolution: internal name conflicts are resolved sanely@generates es",
    "rollup@form@interop-per-dependency-no-freeze: respects the freeze option@generates es",
    "rollup@form@intro-and-outro: adds an intro/outro@generates es",
    "rollup@form@invalid-binary-expressions: Does not fail when bundling code where the `in`-operator is used with invalid right sides",
    "rollup@form@json-parse-is-not-pure: JSON.parse is not pure as it can throw on invalid json strings@generates es",
    "rollup@form@json-stringify-is-not-pure: JSON.stringify is not pure as it can throw on circular structures@generates es",
    "rollup@form@labeled-break-statements: keep break statements if their label is included",
    "rollup@form@labeled-continue-statements: keep continue statements if their label is included",
    "rollup@form@large-var-cnt-deduping: large variable count deduping",
    "rollup@form@mjs: supports loading mjs with precedence@generates es",
    "rollup@form@namespace-conflict: replaces conflicting namespace properties with undefined",
    "rollup@form@namespace-import-reexport-2: properly associate or shadow variables in and around functions@generates es",
    "rollup@form@namespace-import-reexport: properly associate or shadow variables in and around functions@generates es",
    "rollup@form@namespace-object-import: properly encodes reserved names if namespace import is used@generates es",
    "rollup@form@namespace-optimization-b: it does static lookup optimization of internal namespaces, coping with multiple namespaces in one function@generates es",
    "rollup@form@namespace-reexport-name: uses correct names when reexporting from namespace reexports (#4049)@generates es", // the rollup result is simply
    "rollup@form@namespace-self-import: namespace early import hoisting@generates es",
    "rollup@form@namespace-tostring@entry-default: does not add Symbol.toStringTag property to entry chunks with default export mode@generates es",
    "rollup@form@namespace-tostring@entry-named: adds Symbol.toStringTag property to entry chunks with named exports@generates es",
    "rollup@form@namespaced-default-exports: creates namespaced module names@generates es",
    "rollup@form@namespaces-have-null-prototype: creates namespaces with null prototypes@generates es",
    "rollup@form@no-external-live-bindings-compact: Allows omitting the code that handles external live bindings in compact mode@generates es",
    "rollup@form@no-external-live-bindings: Allows omitting the code that handles external live bindings@generates es",
    "rollup@form@ns-external-star-reexport: supports namespaces with external star reexports@generates es",
    "rollup@form@override-external-namespace: allows overriding imports of external namespace reexports@generates es",
    "rollup@form@pattern-member-expressions: handles member expressions in patterns (#2750)",
    "rollup@form@recursive-assignments: do not fail for pathological recursive algorithms and circular structures",
    "rollup@form@recursive-literal-values: do not fail for literal values from recursive return values",
    "rollup@form@relative-external-with-global: applies globals to externalised relative imports@generates es",
    // Passed. convert reexport to import and export
    "rollup@form@reexport-external-default-and-name: reexports a an external default as a name and imports another name from that dependency@generates es",
    "rollup@form@reexport-external-default-and-namespace: reexports a default external import as default export (when using named exports)@generates es",
    "rollup@form@reexport-external-default-as-name-and-name: re-exports a named external export as default@generates es",
    "rollup@form@reexport-external-default: reexports an external default export@generates es",
    "rollup@form@reexport-external-name-as-default2: re-exports a named external export as default via another file@generates es",
    "rollup@form@reexport-external-name-as-default: re-exports a named external export as default@generates es",
    "rollup@form@reexport-external-name: re-exports a named export from an external module@generates es",
    "rollup@form@reexport-external-namespace-as: reexport external namespace as name@generates es",
    "rollup@form@reexport-external-namespace: re-exports * from external module (#791)@generates es",
    "rollup@form@reexport-used-external-namespace-as: reexport external namespace as name if the namespace is also used@generates es",
    "rollup@form@reserved-keywords-in-imports-exports: correctly handles reserved keywords in exports/imports@generates es",
    "rollup@form@top-level-await: top-level await support@generates system",
    "rollup@form@undefined-default-export: handles default exporting undefined",
    "rollup@form@unmodified-default-exports-function-argument: passing unbound default export to function cannot rebind it",
    "rollup@form@yield-expression@missing-space: Inserts space when simplifying yield expression without space",

    // Test is passed. Class related, `class A` -> `var A = class`
    "rollup@form@use-class-name-in-static-block: use the original class name instead of renderName in class body@generates es",
    "rollup@form@static-method-deoptimization: avoids infinite recursions when deoptimizing \"this\" context",
    "rollup@form@reassigned-exported-functions-and-classes: use legal names for exported functions and classed (#1943)@generates es",
    "rollup@form@computed-properties: computed property keys include declarations of referenced identifiers@generates es",
    "rollup@form@dedupes-external-imports: dedupes external imports@generates es",
    "rollup@form@dynamic-import-this-arrow: uses correct \"this\" in dynamic imports when using arrow functions@generates es",
    "rollup@form@dynamic-import-this-function: uses correct \"this\" in dynamic imports when not using arrow functions@generates es",
    "rollup@form@empty-statament-class-member: Do not crash if class body has empty statements@generates es",
    "rollup@form@exported-class-declaration-conflict: handles exporting class declarations with name conflicts in SystemJS@generates es",
    "rollup@form@external-empty-import-no-global: does not expect a global to be provided for empty imports (#1217)@generates es",
    "rollup@form@external-imports: prefixes global names with `global.` when creating UMD bundle (#57)@generates es",
    "rollup@form@super-classes@super-class-prototype-assignment: correctly resolves the prototype of the super class when assigning properites",

    // Passed, but sourcemap/code is different from rollup
    "rollup@function@sourcemap-true-generatebundle: emits sourcemaps before generateBundle hook",
    "rollup@function@sourcemap-inline-generatebundle: includes inline sourcemap comments in generateBundle hook",
    "rollup@form@sourcemaps-external: correct sourcemaps are written (separate file)@generates es", // the mappping is not same as rollup
    "rollup@form@sourcemaps-hidden: correct sourcemaps are written (separate file) without comment@generates es", // the mappping is not same as rollup
    "rollup@sourcemaps@render-chunk-babili: generates valid sourcemap when source could not be determined@generates es", // The rolldown output chunk including `module comment` caused line offset, the rollup provider the fake sourcemap can't remapping.
    "rollup@form@render-chunk-plugin-sourcemaps: supports returning undefined source maps from render chunk hooks, when source maps are enabled@generates es", // the mappping is not same as rollup, the `sources/sourcesContent` perseved original sourcemap is correct
    "rollup@sourcemaps@transform-low-resolution: handles combining low-resolution and high-resolution source-maps when transforming@generates es",// the input string `'bar'`, the rolldown output `"bar"`, caused search original position failed
    "rollup@sourcemaps@names: names are recovered (https://github.com/rollup/rollup/issues/101)@generates es", // the inputs string `Object.create( Bar.prototype )`, the rolldown output `Object.create(Bar.prototype)`, caused search original position failed
    "rollup@sourcemaps@basic-support: basic sourcemap support@generates es",// the inputs string `console.log( 'hello from main.js' )`, the rolldown output `console.log("hello from main.js")`, caused search original position failed

    // passed, the rolldown give a specific warning
    "rollup@function@preload-loading-module: waits for pre-loaded modules that are currently loading",
    // passed, the rolldown using `__name` to keep the original name
    "rollup@form@assignment-to-exports-class-declaration: does not rewrite class expression IDs@generates es",
    "rollup@form@simplify-expression-annotations: keeps correct annotations when simplifying expressinos",

    // passed. but the renaming strategy is different from rollup, causing a mismatch.
    "rollup@form@deconflict-module-priority: prioritizes entry modules over dependencies when deconflicting",
    "rollup@form@reexport-star-deshadow: Star reexports scope deshadowing@generates es"
]
