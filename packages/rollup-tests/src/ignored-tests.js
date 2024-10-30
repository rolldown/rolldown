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
"rollup@function@import-assertions@plugin-assertions-this-resolve: allows plugins to provide attributes for this.resolve",
  "rollup@function@import-assertions@warn-assertion-conflicts: warns for conflicting import attributes",
  "rollup@function@import-assertions@warn-unresolvable-assertions: warns for dynamic import attributes that cannot be resolved",
  // FATAL ERROR: threadsafe_function.rs:573
  'rollup@function@external-ignore-reserved-null-marker: external function ignores \\0 started ids',

  // Need to investigate
  'rollup@function@bundle-facade-order: respects the order of entry points when there are additional facades for chunks',

  // Not supported
  'rollup@function@enforce-plugin-order: allows to enforce plugin hook order',
 
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

  // The threeshake is not working as expected
  "rollup@function@tree-shake-variable-declarations-2: remove unused variables from declarations (#1831)",

  // The dyanmic import inline is not compatible with rollup
  "rollup@function@transparent-dynamic-inlining: Dynamic import inlining when resolution id is a module in the bundle",
  "rollup@function@dynamic-import-existing: Dynamic import inlining when resolution id is a module in the bundle",

  // `PluginContext.resolve` is not working as expected
  "rollup@function@resolve-relative-external-id: resolves relative external ids",

  // The external module is not working as expected
  "rollup@function@relative-external-include-once-nested: includes a relative external module only once (nested version)",
  "rollup@function@relative-external-include-once-two-external: includes a relative external module only once (two external deps)",
  "rollup@function@relative-external-include-once-up: includes a relative external module only once (from upper directory too)",
  "rollup@function@relative-external-include-once: includes a relative external module only once",  // The external module is not working as expected
 
  // The result is not working as expected
  "rollup@function@respect-default-export-reexporter-side-effects: respect side-effects in reexporting modules even if moduleSideEffects are off",
  "rollup@function@respect-reexporter-side-effects: respect side-effects in reexporting modules even if moduleSideEffects are off",
  // The result is not working as expected, Cannot set property dirname of #<Object> which has only a getter
  "rollup@function@override-external-namespace: allows overriding imports of external namespace reexports",
  "rollup@function@override-static-external-namespace: allows overriding imports of external namespace reexports without external live-bindings",

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
  'rollup@function@preserve-modules-default-mode-namespace: import namespace from chunks with default export mode when preserving modules',
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
  // output.manualChunks is not supported
  "rollup@function@manual-chunks-conflict: Throws for conflicts between manual chunks",
  "rollup@function@manual-chunks-include-external-modules3: throws an error EXTERNAL_MODULES_CANNOT_BE_TRANSFORMED_TO_MODULES for manualChunks' modules that are resolved as an external module by the 'external' option",
  "rollup@function@manual-chunks-include-external-modules: throws for manualChunks' modules that are resolved as an external module by plugins",
  "rollup@function@manual-chunks-info: provides additional chunk information to a manualChunks function",
  "rollup@function@circular-namespace-reexport-manual-chunks: correctly handles namespace reexports with circular dependencies when using manual chunks",
  "rollup@function@emit-chunk-manual-asset-source: supports setting asset sources as side effect of the manual chunks option",
  "rollup@function@emit-chunk-manual: supports emitting chunks as side effect of the manual chunks option",
  "rollup@function@inline-imports-with-manual: Manual chunks are not supported when inlining dynamic imports",
  // PluginContext.setAssetSource is not supported
  // Should throw error if asset source is null
  "rollup@function@emit-file@asset-source-invalid2: throws when setting an empty asset source",
  "rollup@function@emit-file@asset-source-invalid3: throws when setting an empty asset source",
  "rollup@function@emit-file@asset-source-invalid4: throws when setting an empty asset source",
  //  Should throw error if PluginContext.setAssetSource set asset source twice
  "rollup@function@emit-file@set-asset-source-twice2: throws when setting the asset source twice",
  "rollup@function@emit-file@set-asset-source-twice: throws when setting the asset source twice",
  // Should throw error if PluginContext.emitFile asset source is null
  "rollup@function@emit-file@asset-source-invalid: throws when setting an empty asset source",
  // PluginContext.getFilename throw error if asset source is not set
  "rollup@function@emit-file@asset-source-missing3: throws when accessing the file name before the asset source is set",
  "rollup@function@emit-file@asset-source-missing4: throws when accessing the file name before the asset source is set",
  // Should throw error if asset source is not set at generate stage
  "rollup@function@emit-file@asset-source-missing2: throws when not setting the asset source",
  "rollup@function@emit-file@asset-source-missing5: throws when not setting the asset source and accessing the asset URL",
  // import.meta.ROLLUP_FILE_URL_<referenceId> throw error if asset source is not set
  "rollup@function@emit-file@asset-source-missing: throws when not setting the asset source",
  // PluginContext.emitFile is not supported emit chunk
  "rollup@function@emit-chunk-hash: gives access to the hashed filed name via this.getFileName in generateBundle",
  "rollup@function@resolveid-is-entry: sends correct isEntry information to resolveId hooks",
  // Should throw error if input option key is `./path` or `/path` or `../path`
  "rollup@function@input-name-validation2: throws for relative paths as input names",
  "rollup@function@input-name-validation3: throws for relative paths as input names",
  "rollup@function@input-name-validation: throws for absolute paths as input names",
  // syntheticNamedExports is not supported
  "rollup@function@synthetic-named-exports-fallback-es2015: adds a fallback in case synthetic named exports are falsy",
  "rollup@function@synthetic-named-exports-fallback: adds a fallback in case synthetic named exports are falsy",
  "rollup@function@synthetic-named-exports@circular-synthetic-exports2: handles circular synthetic exports",
  "rollup@function@synthetic-named-exports@circular-synthetic-exports: handles circular synthetic exports",
  "rollup@function@synthetic-named-exports@dynamic-import: supports dynamically importing a module with synthetic named exports",
  "rollup@function@synthetic-named-exports@entry: does not expose the synthetic namespace if an entry point uses a string value",
  "rollup@function@synthetic-named-exports@external-synthetic-exports: external modules can not have syntheticNamedExports",
  "rollup@function@synthetic-named-exports@namespace-object: does not include named synthetic namespaces in namespace objects",
  "rollup@function@synthetic-named-exports@namespace-overrides: supports re-exported synthetic exports in namespace objects with correct export precedence",
  "rollup@function@synthetic-named-exports@non-default-export: supports providing a named export to generate synthetic exports",
  "rollup@function@synthetic-named-exports@synthetic-exports-need-default: synthetic named exports modules need a default export",
  "rollup@function@synthetic-named-exports@synthetic-exports-need-fallback-export: synthetic named exports modules need their fallback export",
  "rollup@function@synthetic-named-exports@synthetic-named-export-as-default: makes sure default exports of synthetic named exports are snapshots",
  "rollup@function@synthetic-named-exports@synthetic-named-export-entry: does not expose synthetic named exports on entry points",
  "rollup@function@reexport-from-synthetic: handles reexporting a synthetic namespace from a non-synthetic module",
  "rollup@function@respect-synthetic-export-reexporter-side-effects: respect side-effects in reexporting modules even if moduleSideEffects are off",
  // output.generatedCode.symbols is not supported 
  "rollup@function@reexport-ns: external namespace reexport",
  "rollup@function@namespace-tostring@dynamic-import-default-mode: adds Symbol.toStringTag property to dynamic imports of entry chunks with default export mode",
  "rollup@function@namespace-tostring@dynamic-import: adds Symbol.toStringTag property to dynamic imports",
  "rollup@function@namespace-tostring@entry-named: adds Symbol.toStringTag property to entry chunks with named exports",
  "rollup@function@namespace-tostring@external-namespaces: adds Symbol.toStringTag property to external namespaces",
  "rollup@function@namespace-tostring@inlined-namespace: adds Symbol.toStringTag property to inlined namespaces",
  "rollup@function@namespace-tostring@interop-property-descriptor: generated interop namespaces should have correct Symbol.toStringTag",
  "rollup@function@namespace-tostring@property-descriptor: namespace export should have @@toStringTag with correct property descriptors #4336",
  // PluginContext.cache is not supported
  "rollup@function@plugin-cache@anonymous-delete: throws for anonymous plugins deleting from the cache",
  "rollup@function@plugin-cache@anonymous-get: throws for anonymous plugins reading the cache",
  "rollup@function@plugin-cache@anonymous-has: throws for anonymous plugins checking the cache",
  "rollup@function@plugin-cache@anonymous-set: throws for anonymous plugins adding to the cache",
  "rollup@function@plugin-cache@duplicate-names: throws if two plugins with the same name and no cache key access the cache",
  // PluginContext.parse is deprecated
  "rollup@function@plugin-parse-ast-remove-sourcemapping: remove source mapping comment even if code is parsed by PluginContext.parse method",
  "rollup@function@parse-return-outside-function: supports parsing return statements outside functions via options",
  "rollup@function@plugin-parse: plugin transform hooks can use `this.parse(code, options)`",
  "rollup@function@call-marked-pure-with-plugin-parse-ast: external function calls marked with pure comment do not have effects and should be removed even if parsed by PluginContext.parse method",
  // Should check the hook typing is correct
  "rollup@function@non-function-hook-async: throws when providing a value for an async function hook",
  "rollup@function@non-function-hook-sync: throws when providing a value for a sync function hook",
  // The normalziedOptions is not compatible with rollup
  "rollup@function@options-hook: allows to read and modify options in the options hook",
  "rollup@function@output-options-hook: allows to read and modify options in the options hook",
  // maxParallelFileOps is not supported
  "rollup@function@max-parallel-file-operations@default: maxParallelFileOps not set",
  "rollup@function@max-parallel-file-operations@error: maxParallelFileOps: fileRead error is forwarded",
  "rollup@function@max-parallel-file-operations@infinity: maxParallelFileOps set to infinity",
  "rollup@function@max-parallel-file-operations@set: maxParallelFileOps set to 3",
  "rollup@function@max-parallel-file-operations@with-plugin: maxParallelFileOps with plugin",
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
  // Error object is not compatible with rollup
  "rollup@function@plugin-error-with-numeric-code: rollup do not break if get a plugin error that contains numeric code",
  // Should error if call `this.error` at hooks and the error object is not compatible with rollup
  "rollup@function@load-module-error@transform: plugin transform hooks can use `this.error({...}, char)` (#1140)",
  "rollup@function@plugin-error@transform: plugin transform hooks can use `this.error({...}, char)` (#1140)",
  // The warning is not compatible with rollup
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
  // shouldTransformCachedModule hook is not supported
  "rollup@function@plugin-error-should-transform: errors in shouldTransformCachedModule abort the build",
  // PluginContext.load is not supported
  "rollup@function@preload-after-build: supports this.load() in buildEnd and renderStart",
  "rollup@function@preload-cyclic-module: handles pre-loading a cyclic module in the resolveId hook",
  "rollup@function@preload-loading-module: waits for pre-loaded modules that are currently loading",
  "rollup@function@preload-module: allows pre-loading modules via this.load",
  // Give warning if return map or ast without code
  "rollup@function@transform-without-code-warn-ast: warns when returning a map but no code from a transform hook",
  "rollup@function@transform-without-code-warn-map: warns when returning a map but no code from a transform hook",
  // Retrun `meta` from transform hook is not supported
  "rollup@function@transform-without-code: allows using the transform hook for annotations only without returning a code property and breaking sourcemaps",
  // The output.interop is not supported
  "rollup@function@interop-auto-live-bindings: handles interop \"auto\" with live-bindings support",
  "rollup@function@interop-auto-no-live-bindings: handles interop \"auto\" without live-bindings support",
  "rollup@function@interop-default-conflict: handles conflicts with added interop default variables and supports default live bindings",
  "rollup@function@interop-default-only-named-import: throws when using a named import with interop \"defaultOnly\"",
  "rollup@function@interop-default-only-named-namespace-reexport: allows reexporting a namespace as a name when interop is \"defaultOnly\"",
  "rollup@function@interop-default-only-named-reexport: throws when reexporting a namespace with interop \"defaultOnly\"",
  "rollup@function@interop-default-only-namespace-import: allows importing a namespace when interop is \"defaultOnly\"",
  "rollup@function@interop-default-only-namespace-reexport: warns when reexporting a namespace with interop \"defaultOnly\"",
  "rollup@function@interop-default-only: handles interop \"defaultOnly\"",
  "rollup@function@interop-default: handles interop \"default\" with live-bindings support",
  "rollup@function@interop-esmodule: handles interop \"esModule\" with live-bindings support",
  "rollup@function@invalid-interop: throws for invalid interop values",
  // The output.generatedCode.preset is not supported 
  "rollup@function@unknown-generated-code-preset: throws for unknown presets for the generatedCode option",
  // The output.generatedCode is not supported 
  "rollup@function@unknown-generated-code-value: throws for unknown string values for the generatedCode option",
  // The output.treeshake.preset is not supported 
  "rollup@function@unknown-treeshake-preset: throws for unknown presets for the treeshake option",
  // Throws with unknown output.treeshake options
  "rollup@function@unknown-treeshake-value: throws for unknown string values for the treeshake option",
  // Give warning for invalid options or outputOptions
  "rollup@function@warns-for-invalid-options: warns for invalid options",
  // Give warning for invalid treeshake.moduleSideEffects option
  "rollup@function@module-side-effects@invalid-option: warns for invalid options",
  // Throw error for invalid addhook value
  "rollup@function@invalid-addon-hook: throws when providing a non-string value for an addon hook",
  // Throw error for unexpected output exports
  "rollup@function@invalid-default-export-mode: throw for invalid default export mode",
  // Throw error if output.sourcemapIgnoreList return non-boolean value
  "rollup@function@invalid-ignore-list-function: throw descriptive error if sourcemapIgnoreList-function does not return a boolean",
  // Throw error if output.sourcemapPathTransform return non-string value
  "rollup@function@invalid-transform-source-function: throw descriptive error if sourcemapPathTransform-function does not return a string (#3484)",
  // Throw error for invalid placeholder in filename options
  "rollup@function@invalid-pattern-replacement: throws for invalid placeholders in patterns",
  // Throw error for `../xxx` in filename options
  "rollup@function@invalid-pattern: throws for invalid patterns",
  // Throw error for top-level await at format cjs
  "rollup@function@invalid-top-level-await: throws for invalid top-level-await format",
  // The load hook retrun ast is not supported
  "rollup@function@uses-supplied-ast: uses supplied AST",
  // The resolveId hook resolvedBy is not supported
  "rollup@function@validate-resolved-by-logic: validate resolvedBy logic",
  // The `output.validate` is not supported
  "rollup@function@validate-output: handles validate failure",
  // Give warning for empty chunk
  "rollup@function@vars-with-init-in-dead-branch: handles vars with init in dead branch (#1198)",
  // Give parse error for update imported bindings
  "rollup@function@update-expression-of-import-fails: disallows updates to imported bindings",
  "rollup@function@reassign-import-not-at-top-level-fails: disallows assignments to imported bindings not at the top level",
  "rollup@function@reassign-import-fails: disallows assignments to imported bindings",
  // Give warning for unused imports
  "rollup@function@unused-import: warns on unused imports ([#595])",

  // The error/warning msg info is not compatible with rollup
  "rollup@function@throws-not-found-module: throws error if module is not found",
  "rollup@function@shims-missing-exports: shims missing exports",
  "rollup@function@self-referencing-namespace: supports dynamic namespaces that reference themselves",
  "rollup@function@reexport-missing-error: reexporting a missing identifier should print an error",
  "rollup@function@recursive-reexports: handles recursive namespace reexports",
  "rollup@function@paths-are-case-sensitive: insists on correct casing for imports",

  // Shouldn't modify meta objects passed in resolveId hook
  "rollup@function@reuse-resolve-meta: does not modify meta objects passed in resolveId",
  // The `output.paths` is not supported
  "rollup@function@re-export-own: avoid using export.hasOwnProperty",
  // The module information is not compatible with rollup
  "rollup@function@plugin-module-information-no-cache: handles accessing module information via plugins with cache disabled",
  "rollup@function@plugin-module-information: provides module information on the plugin context",

  // Give warns when input hooks are used in output plugins
  "rollup@function@per-output-plugins-warn-hooks: warns when input hooks are used in output plugins",

]

module.exports = {
  ignoreTests,
}
