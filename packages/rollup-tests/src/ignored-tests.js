// cSpell:disable
const ignoreTests = [
  // The giving code is not valid JavaScript.
  'rollup@function@circular-default-exports: handles circular default exports',

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

  // The treeshake is not working as expected
  "rollup@form@tdz-access-in-declaration: detect accessing TDZ variables within the declaration",
  "rollup@function@tree-shake-variable-declarations-2: remove unused variables from declarations (#1831)",
  "rollup@function@can-import-self-treeshake: direct self import", // check chunk why is empty
  "rollup@function@assign-namespace-to-var: allows a namespace to be assigned to a variable",// check chunk why is empty

  // The dyanmic import at format cjs is not compatible with rollup
  // The test passed, but the snapshot is same with rollup
  "rollup@function@transparent-dynamic-inlining: Dynamic import inlining when resolution id is a module in the bundle",
  // output.dynamicImportInCjs is not supported
  "rollup@function@dynamic-import-this-function: uses correct \"this\" in dynamic imports when not using arrow functions",
  "rollup@function@dynamic-import-this-arrow: uses correct \"this\" in dynamic imports when using arrow functions",
  "rollup@function@dynamic-import-expression: Dynamic import expression replacement",

  // The `RenderChunk#modules` should ignores non-bundled modules
  "rollup@function@inline-dynamic-imports-bundle: ignores non-bundled modules when inlining dynamic imports",
 
  // The result is not working as expected
  "rollup@function@respect-default-export-reexporter-side-effects: respect side-effects in reexporting modules even if moduleSideEffects are off",
  "rollup@function@respect-reexporter-side-effects: respect side-effects in reexporting modules even if moduleSideEffects are off",
  "rollup@function@non-js-extensions: non .js extensions are preserved",
  "rollup@function@no-external-live-bindings: Allows omitting the code that handles external live bindings",
  "rollup@function@no-external-live-bindings-compact: Allows omitting the code that handles external live bindings",
  "rollup@function@namespace-member-side-effects@unknown-access: respects side effects when accessing unknown namespace members",
  "rollup@function@namespace-member-side-effects@assignment: checks side effects when reassigning namespace members",
  "rollup@function@name-conflict-promise: avoids name conflicts with local variables named Promise",
  "rollup@function@module-side-effects@writable: ModuleInfo.moduleSideEffects should be writable during build time",
  "rollup@function@module-side-effects@transform: handles setting moduleSideEffects in the transform hook",
  "rollup@function@module-side-effects@resolve-id-external: does not include modules without used exports if moduleSideEffect is false",
  "rollup@function@module-side-effects@resolve-id: does not include modules without used exports if moduleSideEffect is false",
  "rollup@function@module-side-effects@load: handles setting moduleSideEffects in the load hook",
  "rollup@function@module-side-effects@external-false: supports setting module side effects to false for external modules",
  "rollup@function@module-side-effects@array: supports setting module side effects via an array",
  "rollup@function@module-side-effect-reexport: includes side effects of re-exporters unless they have moduleSideEffects: false",
  "rollup@function@hoisted-variable-if-else: handles hoisted variables in chained if statements",
  "rollup@function@external-conflict: external paths from custom resolver remain external (#633)",
  "rollup@function@external-live-binding-compact: handles external live-bindings",
  "rollup@function@external-live-binding: handles external live-bindings",
  "rollup@function@external-dynamic-import-live-binding-compact: supports external dynamic imports with live bindings in compact mode",
  "rollup@function@external-dynamic-import-live-binding: supports external dynamic imports with live bindings",
  "rollup@function@argument-deoptimization@global-calls: tracks argument mutations of calls to globals",
  "rollup@form@export-all-before-named: external `export *` must not interfere with internal exports@generates es",
  "rollup@form@export-all-multiple: correctly handles multiple export * declarations (#1252)@generates es",
  "rollup@form@hoisted-vars-in-dead-branches: renders hoisted variables in dead branches", // https://github.com/oxc-project/oxc/issues/7209
  "rollup@form@mutations-in-imports: track mutations of imports",
 
  // The `this` related
  "rollup@form@proper-this-context: make sure \"this\" respects the context for arrow functions", 
  "rollup@form@this-is-undefined: top-level `this` expression is rewritten as `undefined`@generates es",

  // `return init_foo(), foo_exports;` is not expected 
  "rollup@form@dynamic-import-inlining: dynamic import inlining",
  "rollup@form@dynamic-import-inlining-array: supports an array with a single entry when inlining dynamic imports",
  "rollup@form@inline-with-reexport: handles inlining dynamic imports when the imported module contains reexports",
  "rollup@form@nested-inlined-dynamic-import: deconflicts variables when nested dynamic imports are inlined@generates es",

  // /*@__PURE__*/ related
  "rollup@form@pure-comment-scenarios-complex: correctly handles various advanced pure comment scenarios",
  "rollup@form@nested-pure-comments: correctly associates pure comments before sequence expressions etc.", 
  // treeshake.annotations false is not supported
  "rollup@form@pure-comments-disabled: does not rely on pure annotations if they are disabled",

  // deconfilct
  "rollup@function@deshadow-respect-existing: respect existing variable names when deshadowing",
  "rollup@function@class-name-conflict-2: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict-3: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict-4: does not shadow variables when preserving class names",
  "rollup@function@class-name-conflict: preserves class names even if the class is renamed",
  "rollup@form@assignment-to-exports-class-declaration: does not rewrite class expression IDs@generates es",
  "rollup@form@body-less-for-loops: supports body-less for loops",// rollup not deconflict
  "rollup@form@catch-parameter-shadowing: the parameter of a catch block should correctly shadow an import (#1391)",
  "rollup@form@import-specifier-deshadowing: deshadows aliased import bindings@generates es",

  // comment related
  "rollup@form@comment-before-import: preserves comments before imports@generates es",
  "rollup@form@comment-start-inside-comment: properly remove coments above import statements@generates es",

  // Format cjs
  "rollup@function@default-export-with-null-prototype: default exports of objects with null prototypes are supported",

  // The result is not working as expected, Cannot set property dirname of #<Object> which has only a getter
  "rollup@function@override-external-namespace: allows overriding imports of external namespace reexports",
  "rollup@function@override-static-external-namespace: allows overriding imports of external namespace reexports without external live-bindings",

  // Logging is not working as expected
  "rollup@function@logging@handle-logs-in-plugins: allows plugins to read and filter logs",
  "rollup@function@logging@log-from-options: can log from the options hook",
  "rollup@function@logging@plugin-order: allows to order plugins when logging",
  "rollup@function@logging@promote-log-to-error: allows turning logs into errors",

  // The output plugins hooks is not working as expected
  "rollup@function@options-in-renderstart: makes input and output options available in renderStart",

  // Nested plugin is not supported
  "rollup@function@nested-and-async-plugin: works when nested plugin",

  // The output code/sourcemap is not same as rollup,
  "rollup@function@sourcemap-true-generatebundle: emits sourcemaps before generateBundle hook",
  "rollup@function@sourcemap-inline-generatebundle: includes inline sourcemap comments in generateBundle hook",
  // invalid output.exports should not panic
  "rollup@function@export-type-mismatch-b: export type must be auto, default, named or none",

  // The input option is emtpy string
  "rollup@function@avoid-variable-be-empty: avoid variable from empty module name be empty",
 
  // import.meta.ROLLUP_FILE_URL_<referenceId> is not supported
  "rollup@function@emit-file@file-references-in-bundle: lists referenced files in the bundle",
  "rollup@form@emit-asset-file: supports emitting assets from plugin hooks@generates es",
  "rollup@form@emit-uint8array-no-buffer: supports emitting assets as Uint8Arrays when Buffer is not available@generates es",

  // Should check the hook typing is correct
  "rollup@function@non-function-hook-async: throws when providing a value for an async function hook",
  "rollup@function@non-function-hook-sync: throws when providing a value for a sync function hook",
  // The normalziedOptions is not compatible with rollup
  "rollup@function@options-hook: allows to read and modify options in the options hook",
  "rollup@function@output-options-hook: allows to read and modify options in the options hook",

  // Should error if call `this.error` at hooks
  "rollup@function@plugin-error@buildEnd: buildStart hooks can use this.error",
  "rollup@function@plugin-error@buildStart: buildStart hooks can use this.error",
  "rollup@function@plugin-error@generateBundle: buildStart hooks can use this.error",
  "rollup@function@plugin-error@load: buildStart hooks can use this.error",
  "rollup@function@plugin-error@renderChunk: buildStart hooks can use this.error",
  "rollup@function@plugin-error@renderStart: buildStart hooks can use this.error",
  "rollup@function@plugin-error@resolveId: buildStart hooks can use this.error",
  "rollup@function@load-module-error@buildEnd: buildStart hooks can use this.error",
  "rollup@function@load-module-error@buildStart: buildStart hooks can use this.error",
  "rollup@function@load-module-error@generateBundle: buildStart hooks can use this.error",
  "rollup@function@load-module-error@renderChunk: buildStart hooks can use this.error",
  "rollup@function@load-module-error@renderStart: buildStart hooks can use this.error",
  "rollup@function@load-module-error@resolveId: buildStart hooks can use this.error",
  "rollup@function@logging@this-error-onlog: can turn logs into errors via this.error in the onLog hook",
  "rollup@function@plugin-error-only-first-render-chunk: throws error only with first plugin renderChunk",
  "rollup@function@plugin-error-only-first-transform: throws error only with first plugin transform",
  "rollup@function@plugin-error-module-parsed: errors in moduleParsed abort the build",
  // PluginContext.error accpet more arguments with transform hooks 
  "rollup@function@plugin-error-transform-pos: `this.error(...)` accepts number as second parameter (#5044)",
  "rollup@function@plugin-error-loc-instead-pos: `this.error(...)` accepts { line, column } object as second parameter (#1265)",
 
  // Should error if call `this.error` at hooks and the error object is not compatible with rollup
  "rollup@function@load-module-error@transform: plugin transform hooks can use `this.error({...}, char)` (#1140)",
  "rollup@function@plugin-error@transform: plugin transform hooks can use `this.error({...}, char)` (#1140)",
  
  // Module meta related
  // Shouldn't modify meta objects passed in resolveId hook
  "rollup@function@reuse-resolve-meta: does not modify meta objects passed in resolveId",
  "rollup@function@modify-meta: allows to freely modify moduleInfo.meta and maintain object identity",
  "rollup@function@custom-module-options: supports adding custom options to modules",
  "rollup@function@custom-external-module-options: supports adding custom options to external modules",

  // The `output.file` is not supported
  "rollup@function@file-and-dir: throws when using both the file and the dir option",

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
  "rollup@form@sourcemaps-external: correct sourcemaps are written (separate file)@generates es", // file not expected
  "rollup@form@sourcemaps-hidden: correct sourcemaps are written (separate file) without comment@generates es", // file not expected

  // Passed, but the output snapshot is same as rollup
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

  // The error/warning msg info is not compatible with rollup
  // TODO check the error is not break bundle
  "rollup@function@throws-not-found-module: throws error if module is not found",
  "rollup@function@shims-missing-exports: shims missing exports",
  "rollup@function@self-referencing-namespace: supports dynamic namespaces that reference themselves",
  "rollup@function@reexport-missing-error: reexporting a missing identifier should print an error",
  "rollup@function@recursive-reexports: handles recursive namespace reexports",
  "rollup@function@paths-are-case-sensitive: insists on correct casing for imports",
  "rollup@function@no-relative-external: missing relative imports are an error, not a warning",
  "rollup@function@namespace-update-import-fails: disallows updates to namespace exports",
  "rollup@function@namespace-reassign-import-fails: warns for reassignments to namespace exports",
  "rollup@function@namespace-missing-export: replaces missing namespace members with undefined and warns about them",
  "rollup@function@warn-misplaced-annotations: warns for misplaced annotations",
  "rollup@function@warn-missing-iife-name: warns if no name is provided for an IIFE bundle",
  "rollup@function@warn-on-auto-named-default-exports: warns if default and named exports are used in auto mode",
  "rollup@function@warn-on-empty-bundle: warns if empty bundle is generated  (#444)",
  "rollup@function@warn-on-eval: warns about use of eval",
  "rollup@function@warn-on-namespace-conflict: warns on duplicate export * from",
  "rollup@function@warn-on-top-level-this: warns on top-level this (#770)",
  "rollup@function@warn-on-unused-missing-imports: warns on missing (but unused) imports",
  "rollup@function@warning-incorrect-sourcemap-location: does not fail if a warning has an incorrect location due to missing sourcemaps",
  "rollup@function@warning-low-resolution-location: handles when a low resolution sourcemap is used to report an error",
  "rollup@function@warnings-to-string: provides a string conversion for warnings",
  "rollup@function@plugin-error-with-numeric-code: rollup do not break if get a plugin error that contains numeric code",
  "rollup@function@load-module-error@load: throws when a module cannot be loaded",
  "rollup@function@inline-imports-with-multiple-object: Having multiple inputs in an object is not supported when inlining dynamic imports",
  "rollup@function@inline-imports-with-multiple-array: Having multiple inputs in an array is not supported when inlining dynamic imports",
  "rollup@function@import-of-unexported-fails: marking an imported, but unexported, identifier should throw",
  "rollup@function@iife-code-splitting: throws when generating multiple chunks for an IIFE build",
  "rollup@function@external-entry-point: throws for entry points that are resolved as false by plugins",
  "rollup@function@external-entry-point-object: throws for entry points that are resolved as an external object by plugins",
  "rollup@function@export-type-mismatch-c: cannot have named exports if explicit export type is default",
  "rollup@function@export-type-mismatch: cannot have named exports if explicit export type is default",
  "rollup@function@error-parse-json: throws with an extended error message when failing to parse a file with \".json\" extension",
  "rollup@function@error-parse-unknown-extension: throws with an extended error message when failing to parse a file without .(m)js extension",
  "rollup@function@error-missing-umd-name: throws an error if no name is provided for a UMD bundle",
  "rollup@function@error-after-transform-should-throw-correct-location: error after transform should throw with correct location of file",
  "rollup@function@dynamic-import-relative-not-found: throws if a dynamic relative import is not found",
  "rollup@function@dynamic-import-not-found: warns if a dynamic import is not found",
  "rollup@function@does-not-hang-on-missing-module: does not hang on missing module (#53)",
  "rollup@function@default-not-reexported: default export is not re-exported with export *",
  "rollup@function@banner-and-footer: adds a banner/footer",
  "rollup@function@circular-missed-reexports-2: handles circular reexports",
  "rollup@function@circular-missed-reexports: handles circular reexports",
  "rollup@function@check-resolve-for-entry: checks that entry is resolved",
  "rollup@function@cycles-export-star: does not stack overflow on `export * from X` cycles",
  "rollup@function@cycles-defaults: cycles work with default exports",
  "rollup@function@cycles-stack-overflow: does not stack overflow on crazy cyclical dependencies",
  "rollup@function@cycles-default-anonymous-function-hoisted: Anonymous function declarations are hoisted",
  "rollup@function@cycles-immediate: handles cycles where imports are immediately used",
  "rollup@function@cycles-pathological-2: resolves even more pathological cyclical dependencies gracefully",
  "rollup@function@custom-path-resolver-plural-b: resolver error is not caught",
  "rollup@function@conflicting-reexports@named-import: throws when a conflicting binding is imported via a named import",
  "rollup@function@conflicting-reexports@named-import-external: warns when a conflicting binding is imported via a named import from external namespaces",
  "rollup@function@can-import-self: a module importing its own bindings",
  "rollup@function@already-deshadowed-import: handle already module import names correctly if they are have already been deshadowed",
  "rollup@function@nested-inlined-dynamic-import-2: deconflicts variables when nested dynamic imports are inlined",

  // The treeshaking is not compatible with rollup
  "rollup@form@conditional-put-parens-around-sequence: put parens around sequences if conditional simplified (#1311)",
  "rollup@form@for-in-scopes: properly associate or shadow variables in and around for-in-loops", // the treeshaking affect deconfilct
  "rollup@form@curried-function: properly handles a curried function", // the treeshaking affect deconfilct
  "rollup@form@early-bind-member-expressions: correctly resolves namespace members when accessed early (#2895)", // `const {x} = xxx`, x unused
  "rollup@form@effect-in-for-of-loop: includes effects in for-of loop (#870)@generates es",// `const x = xxx`, x unused
  "rollup@form@for-loop-assignment: removes assignments with computed indexes in for loops",
  "rollup@form@for-of-scopes: properly associate or shadow variables in and around for-of-loops",
  "rollup@form@for-scopes: properly associate or shadow variables in and around for-loops@generates es",
  "rollup@form@function-mutation: function-mutations do not have effects@generates es",
  "rollup@form@function-scopes: properly associate or shadow variables in and around functions@generates es", //the treeshaking affect deconfilct
  "rollup@form@getter-return-values: forwards return values of getters",
  "rollup@form@super-classes@super-class-prototype-access: correctly resolves the prototype of the super class when accessing properties",
  "rollup@form@import-named-exported-global-with-alias: allow globals to be exported and imported",
  "rollup@form@literals-from-class-statics: tracks literal values in class static fields", // minify feature
  "rollup@form@logical-expression@mutate-logical-expression: properly handle the results of mutating logical expressions@generates es",
  "rollup@form@logical-expression@simplify-non-boolean: simplifies logical expressions that resolve statically to non-boolean values", //  minify feature
  "rollup@form@namespace-missing-export-effects: handles interacting with missing namespace members", // the cross module const folding
  "rollup@form@namespace-optimization-computed-string: it does dynamic lookup optimization of internal namespaces for string-literal keys@generates es",
  "rollup@form@nested-this-expressions: properly keep or ignore nested \"this\"-expressions",
  "rollup@form@object-expression@proto-property: Deoptimize when __proto__ is used", // minify feature
  "rollup@form@optional-chaining: supports optional chaining", // minify feature
  "rollup@form@property-setters-and-getters@early-access-getter-return: handles accessing the return expression of a getter before it has been bound",
  "rollup@form@property-setters-and-getters@early-access-getter-value: handles accessing the value of a getter before it has been bound",
  "rollup@form@property-setters-and-getters@shadowed-setters: handles setters shadowed by computed setters",
  "rollup@form@prototype-functions: properly includes prototype functions",// const folding
  "rollup@form@redeclarations: make sure re-declarations via var and function are linked properly",
  "rollup@form@render-removed-statements: make sure removed statements do no leave unwanted white-space",
  "rollup@form@simplify-return-expression: Simplifies conditionals in return expression",
  "rollup@form@switch-scopes: correctly handles switch scopes",
  "rollup@form@tdz-pattern-access: handles accessing variables declared in patterns before their declaration",
  "rollup@form@this-in-imports: properly keep or ignore \"this\"-expressions when calling imported functions",
  "rollup@form@unmodified-default-exports: does not treat property assignment as rebinding for sake of unbound default exports",
  "rollup@form@wrap-simplified-expressions: wraps simplified expressions that have become callees if necessary@generates es", // const folding
  "rollup@form@try-statement-deoptimization@supports-core-js: supports core-js feature detection (#2869)",
]

// Generated by packages/rollup-tests/test/form/found-tree-shaking-not-align.js
const ignoredTreeshakingTests = require('./ignored-treeshaking-tests.json')

module.exports = {
  ignoreTests: ignoreTests.concat(ignoredTreeshakingTests),
}
