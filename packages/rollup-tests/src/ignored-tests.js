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
  "rollup@function@module-parsed-imported-ids: provides full importedIds and dynamicallyImportedIds in the moduleParsed hook",
  "rollup@function@hoisted-variable-if-else: handles hoisted variables in chained if statements",
  "rollup@function@facade-reexports: handles reexports when creating a facade chunk and transitive dependencies are not hoisted",
  "rollup@function@external-resolved: passes both unresolved and resolved ids to the external option",
  "rollup@function@external-conflict: external paths from custom resolver remain external (#633)",
  "rollup@function@external-live-binding-compact: handles external live-bindings",
  "rollup@function@external-live-binding: handles external live-bindings",
  "rollup@function@external-dynamic-import-live-binding-compact: supports external dynamic imports with live bindings in compact mode",
  "rollup@function@external-dynamic-import-live-binding: supports external dynamic imports with live bindings",
  "rollup@function@duplicate-input-entry: handles duplicate entry modules when using the object form",
  "rollup@function@argument-deoptimization@global-calls: tracks argument mutations of calls to globals",
  "rollup@form@export-all-before-named: external `export *` must not interfere with internal exports@generates es",
  "rollup@form@export-all-multiple: correctly handles multiple export * declarations (#1252)@generates es",
  "rollup@form@guessed-global-names: guesses global names if necessary@generates es",
  "rollup@form@hoisted-vars-in-dead-branches: renders hoisted variables in dead branches", // https://github.com/oxc-project/oxc/issues/7209
  "rollup@form@mutations-in-imports: track mutations of imports",

  // The rolldown import external module is tree-shaked at format iife/umd
  "rollup@form@handles-empty-imports-iife: handles empty imports when generating IIFE output", 
  "rollup@form@handles-empty-imports-umd: handles empty imports when generating IIFE output",

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

  // `makeAbsoluteExternalsRelative` is not supported
  "rollup@function@resolve-relative-external-id: resolves relative external ids",
  "rollup@function@relative-external-include-once-nested: includes a relative external module only once (nested version)",
  "rollup@function@relative-external-include-once-two-external: includes a relative external module only once (two external deps)",
  "rollup@function@relative-external-include-once-up: includes a relative external module only once (from upper directory too)",
  "rollup@function@relative-external-include-once: includes a relative external module only once",
  "rollup@function@external-directory-import: handles using ../ as external import (#4349)", // makeAbsoluteExternalsRelative normlized the external id to absolute path, and renormalize to renderPath https://github.com/rollup/rollup/blob/master/src/ExternalChunk.ts#L51
  "rollup@function@configure-relative-external-module: allows a nonexistent relative module to be configured as external",
  "rollup@form@make-absolute-externals-relative@make-relative-false: does not normalize external paths when set to false",
  "rollup@form@make-absolute-externals-relative@make-relative-relative: only normalizes external paths that were originally relative when set to \"ifRelativeSource\"",
  "rollup@form@make-absolute-externals-relative@make-relative-true: normalizes both relative and absolute external paths when set to true",
  "rollup@form@relative-external-ids: relative external ids are absolutely resolved",

  // escaping external id is not supported
  "rollup@form@quote-id: handles escaping for external ids@generates es",

  // `resolveDynamicImport` hook `specifier: AstNode` not supported
  "rollup@form@dynamic-import-unresolvable: Returns the raw AST nodes for unresolvable dynamic imports@generates es",

  // The plugin sequential is not supported
  "rollup@function@enforce-sequential-plugin-order: allows to enforce sequential plugin hook order for parallel plugin hooks",


  // `renderDynamicImport/resolveFileUrl/resolveImportMeta/shouldTransformCachedModule` hooks not supported
  'rollup@function@enforce-plugin-order: allows to enforce plugin hook order',
 
  // `renderDynamicImport` hook not supported
  "rollup@form@custom-dynamic-import-no-interop: does not add any interop when using a custom dynamic import handler@generates es",

  // `resolveFileUrl` hook not supported
  "rollup@form@configure-file-url: allows to configure file urls@generates es",

  // The output plugins hooks is not working as expected
  "rollup@function@options-in-renderstart: makes input and output options available in renderStart",

  // Nested plugin is not supported
  "rollup@function@nested-and-async-plugin: works when nested plugin",

  // Import Assertions is not supported
  'rollup@function@import-assertions@plugin-assertions-this-resolve: allows plugins to provide assertions for this.resolve',
  "rollup@function@import-assertions@plugin-assertions-this-resolve: allows plugins to provide attributes for this.resolve",
  "rollup@function@import-assertions@warn-assertion-conflicts: warns for conflicting import attributes",
  "rollup@function@import-assertions@warn-unresolvable-assertions: warns for dynamic import attributes that cannot be resolved",
  "rollup@form@deprecated@removes-dynamic-assertions: keep import assertions for dynamic imports",
  "rollup@form@deprecated@removes-static-attributes: keeps any import assertions on input",
  
  // Import attributes is not supported
  "rollup@form@import-attributes@attribute-shapes: handles special shapes of attributes",
  "rollup@form@import-attributes@keep-dynamic-assertions: keep import attributes for dynamic imports@generates es",
  "rollup@form@import-attributes@keep-dynamic-attributes: keep import attributes for dynamic imports@generates es",
  "rollup@form@import-attributes@keeps-static-assertions: keeps any import assertions on input@generates es",
  "rollup@form@import-attributes@keeps-static-attributes: keeps any import attributes on input@generates es",
  "rollup@form@import-attributes@plugin-attributes-resolvedynamicimport: allows plugins to read and write import attributes in resolveDynamicImport",
  "rollup@form@import-attributes@plugin-attributes-resolveid: allows plugins to read and write import attributes in resolveId",
  "rollup@form@import-attributes@removes-dynamic-attributes: keep import attributes for dynamic imports",
  "rollup@form@import-attributes@removes-static-attributes: keeps any import attributes on input",
  
  // output.sourcemapExcludeSources is not supported
  "rollup@form@sourcemaps-excludesources: correct sourcemaps are written (excluding sourceContent)@generates es",
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
  // output.preserveEntrySignatures is not supported
  "rollup@function@dynamic-imports-shared-exports: allows sharing imports between dynamic chunks",

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
  // Cannot emit files or set asset sources in the "outputOptions/transform" hook
  "rollup@function@emit-file@set-asset-source-transform: throws when setting the asset source in the transform hook",
  "rollup@function@emit-file@set-source-in-output-options: throws when trying to set file sources in  the outputOptions hook",
  //  Should throw error if PluginContext.setAssetSource set asset source twice
  "rollup@function@emit-file@set-asset-source-twice2: throws when setting the asset source twice",
  "rollup@function@emit-file@set-asset-source-twice: throws when setting the asset source twice",
  // Should throw error if PluginContext.emitFile asset source is null
  "rollup@function@emit-file@asset-source-invalid: throws when setting an empty asset source",
  // Should throw error if asset source id is invalid
  "rollup@function@emit-file@invalid-set-asset-source-id: throws for invalid asset ids",
  // PluginContext.getFilename throw error if asset source is not set
  "rollup@function@emit-file@asset-source-missing3: throws when accessing the file name before the asset source is set",
  "rollup@function@emit-file@asset-source-missing4: throws when accessing the file name before the asset source is set",
  // Should throw error if asset source is not set at generate stage
  "rollup@function@emit-file@asset-source-missing2: throws when not setting the asset source",
  "rollup@function@emit-file@asset-source-missing5: throws when not setting the asset source and accessing the asset URL",
  // import.meta.ROLLUP_FILE_URL_<referenceId> is not supported
  "rollup@function@emit-file@file-references-in-bundle: lists referenced files in the bundle",
  "rollup@form@emit-asset-file: supports emitting assets from plugin hooks@generates es",
  "rollup@form@emit-uint8array-no-buffer: supports emitting assets as Uint8Arrays when Buffer is not available@generates es",
  // import.meta.ROLLUP_FILE_URL_<referenceId> throw error if asset source is not set
  "rollup@function@emit-file@asset-source-missing: throws when not setting the asset source",
  // import.meta.ROLLUP_FILE_URL_<referenceId> throw error if invalid reference id
  "rollup@function@emit-file@invalid-reference-id: throws for invalid reference ids",

  // PluginContext.emitFile emit chunk is not supported 
  "rollup@function@emit-chunk-hash: gives access to the hashed filed name via this.getFileName in generateBundle",
  "rollup@function@resolveid-is-entry: sends correct isEntry information to resolveId hooks",
  "rollup@function@inline-dynamic-no-treeshake: handles inlining dynamic imports when treeshaking is disabled for modules (#4098)",
  "rollup@function@implicit-dependencies@dependant-dynamic-import-no-effects: throws when a module that is loaded before an emitted chunk is fully tree-shaken",
  "rollup@function@implicit-dependencies@dependant-dynamic-import-not-included: throws when a module that is loaded before an emitted chunk is only linked to the module graph via a tree-shaken dynamic import",
  "rollup@function@implicit-dependencies@dependant-not-part-of-graph: throws when a module that is loaded before an emitted chunk is not part of the module graph",
  "rollup@function@implicit-dependencies@external-dependant: throws when a module that is loaded before an emitted chunk does not exist",
  "rollup@function@implicit-dependencies@missing-dependant: throws when a module that is loaded before an emitted chunk is external",
  "rollup@function@emit-file@set-asset-source-chunk: throws when trying to set the asset source of a chunk",
  "rollup@function@emit-file@no-input: It is not necessary to provide an input if a dynamic entry is emitted",
  "rollup@function@emit-file@modules-loaded: Throws when adding a chunk after the modules have finished loading",
  "rollup@function@emit-file@invalid-chunk-id: throws for invalid chunk ids",
  "rollup@function@emit-file@chunk-not-found: Throws if an emitted entry chunk cannot be resolved",
  "rollup@function@emit-file@chunk-filename-not-available-buildEnd: Throws when accessing the filename before it has been generated in buildEnd",
  "rollup@function@emit-file@chunk-filename-not-available-renderStart: Throws when accessing the filename before it has been generated in renderStart",
  "rollup@function@emit-file@chunk-filename-not-available: Throws when accessing the filename before it has been generated",

  // PluginContext.emitFile emit prebuilt chunk is not supported 
  "rollup@function@emit-file@prebuilt-chunk: get right prebuilt chunks",
  "rollup@function@emit-file@invalid-prebuilt-chunk-filename: throws for invalid prebuilt chunks filename",
  "rollup@function@emit-file@invalid-prebuit-chunk-code: throws for invalid prebuilt chunks code",

  // Should throw error if input option key is `./path` or `/path` or `../path`
  "rollup@function@input-name-validation2: throws for relative paths as input names",
  "rollup@function@input-name-validation3: throws for relative paths as input names",
  "rollup@function@input-name-validation: throws for absolute paths as input names",
  // syntheticNamedExports is not supported
  "rollup@form@synthetic-named-exports: synthetic named exports",
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
  "rollup@function@internal-reexports-from-external: supports namespaces with external star reexports",
  "rollup@function@deconflict-synthetic-named-export-cross-chunk: deconflicts synthetic named exports across chunks",
  "rollup@function@deconflict-synthetic-named-export: deconflicts synthetic named exports",
  "rollup@form@entry-with-unused-synthetic-exports: does not include unused synthetic namespace object in entry points@generates es",
  "rollup@form@merge-namespaces-non-live: merges namespaces without live-bindings",
  "rollup@form@merge-namespaces: merges namespaces with live-bindings",

  // output.generatedCode is not supported 
  "rollup@form@generated-code-compact@arrow-functions-false: does not use arrow functions@generates es",
  "rollup@form@generated-code-compact@arrow-functions-true: uses arrow functions@generates es",
  "rollup@form@generated-code-compact@const-bindings-false: does not use block bindings@generates es",
  "rollup@form@generated-code-compact@const-bindings-true: uses block bindings@generates es",
  "rollup@form@generated-code-compact@object-shorthand-false: does not use object shorthand syntax",
  "rollup@form@generated-code-compact@object-shorthand-true: uses object shorthand syntax",
  "rollup@form@generated-code-compact@reserved-names-as-props-false: escapes reserved names used as props@generates es",
  "rollup@form@generated-code-compact@reserved-names-as-props-true: escapes reserved names used as props@generates es",
  "rollup@form@generated-code-presets@es2015: handles generatedCode preset \"es2015\"",
  "rollup@form@generated-code-presets@es5: handles generatedCode preset \"es5\"",
  "rollup@form@generated-code-presets@preset-with-override: handles generatedCode preset \"es2015\"",
  "rollup@form@generated-code@arrow-functions-false: does not use arrow functions@generates es",
  "rollup@form@generated-code@arrow-functions-true: uses arrow functions@generates es",
  "rollup@form@generated-code@const-bindings-false: does not use block bindings@generates es",
  "rollup@form@generated-code@const-bindings-true: uses block bindings@generates es",
  "rollup@form@generated-code@object-shorthand-false: does not use object shorthand syntax",
  "rollup@form@generated-code@object-shorthand-true: uses object shorthand syntax",
  "rollup@form@generated-code@reserved-names-as-props-false: escapes reserved names used as props@generates es",
  "rollup@form@generated-code@reserved-names-as-props-true: escapes reserved names used as props@generates es",
  // output.generatedCode.symbols is not supported 
  "rollup@function@reexport-ns: external namespace reexport",
  "rollup@function@namespace-tostring@dynamic-import-default-mode: adds Symbol.toStringTag property to dynamic imports of entry chunks with default export mode",
  "rollup@function@namespace-tostring@dynamic-import: adds Symbol.toStringTag property to dynamic imports",
  "rollup@function@namespace-tostring@entry-named: adds Symbol.toStringTag property to entry chunks with named exports",
  "rollup@function@namespace-tostring@external-namespaces: adds Symbol.toStringTag property to external namespaces",
  "rollup@function@namespace-tostring@inlined-namespace: adds Symbol.toStringTag property to inlined namespaces",
  "rollup@function@namespace-tostring@interop-property-descriptor: generated interop namespaces should have correct Symbol.toStringTag",
  "rollup@function@namespace-tostring@property-descriptor: namespace export should have @@toStringTag with correct property descriptors #4336",
  "rollup@function@name-conflict-symbol: avoids name conflicts with local variables named Symbol", // the `Symbol` need to deconflict
  "rollup@form@namespace-tostring@inlined-namespace-static-resolution: statically resolves Symbol.toStringTag for inlined namespaces",
  "rollup@form@namespace-tostring@inlined-namespace: adds Symbol.toStringTag property to inlined namespaces@generates es",

  // PluginContext.cache is not supported
  "rollup@function@plugin-cache@anonymous-delete: throws for anonymous plugins deleting from the cache",
  "rollup@function@plugin-cache@anonymous-get: throws for anonymous plugins reading the cache",
  "rollup@function@plugin-cache@anonymous-has: throws for anonymous plugins checking the cache",
  "rollup@function@plugin-cache@anonymous-set: throws for anonymous plugins adding to the cache",
  "rollup@function@plugin-cache@duplicate-names: throws if two plugins with the same name and no cache key access the cache",
  // Bundle.cache is not supported
  "rollup@function@module-tree: bundle.modules includes dependencies (#903)",
  "rollup@function@has-modules-array: user-facing bundle has modules array",

  // PluginContext.parse is deprecated
  "rollup@function@plugin-parse-ast-remove-sourcemapping: remove source mapping comment even if code is parsed by PluginContext.parse method",
  "rollup@function@parse-return-outside-function: supports parsing return statements outside functions via options",
  "rollup@function@plugin-parse: plugin transform hooks can use `this.parse(code, options)`",
  "rollup@function@call-marked-pure-with-plugin-parse-ast: external function calls marked with pure comment do not have effects and should be removed even if parsed by PluginContext.parse method",
  "rollup@function@handle-missing-export-source: does not fail if a pre-generated AST is omitting the source property of an unused named export (#3210)",
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
 
  // Should error if call `this.error` at hooks and the error object is not compatible with rollup
  "rollup@function@load-module-error@transform: plugin transform hooks can use `this.error({...}, char)` (#1140)",
  "rollup@function@plugin-error@transform: plugin transform hooks can use `this.error({...}, char)` (#1140)",
  
  // shouldTransformCachedModule hook is not supported
  "rollup@function@plugin-error-should-transform: errors in shouldTransformCachedModule abort the build",
  
  // PluginContext.load is not fully supported
  "rollup@form@supports-core-js: supports core-js", // `@rollup/plugin-commonjs` is not supported
  "rollup@form@supports-es5-shim: supports es5-shim", // `@rollup/plugin-commonjs` is not supported
  "rollup@form@supports-es6-shim: supports es6-shim", // `@rollup/plugin-commonjs` is not supported
  "rollup@function@preload-cyclic-module: handles pre-loading a cyclic module in the resolveId hook", // load entry module at resolveId hook
  "rollup@function@preload-module: allows pre-loading modules via this.load", // load entry module at resolveId hook

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
  "rollup@function@deconflicts-interop: deconflicts the interop function",
  "rollup@form@interop-per-dependency-no-live-binding: allows to configure the interop type per external dependency",
  "rollup@form@interop-per-dependency: allows to configure the interop type per external dependency@generates es",
  "rollup@form@interop-per-reexported-dependency: allows to configure the interop type per reexported external dependency@generates es",

  // The `output.globals` function is not supported
  "rollup@form@globals-function: Externals aliases with deshadowing@generates es",
  "rollup@form@external-imports-custom-names-function: allows globals to be specified as a function@generates es",

  // The load hook retrun ast is not supported
  "rollup@function@uses-supplied-ast: uses supplied AST",

  // The resolveId hook resolvedBy is not supported
  "rollup@function@validate-resolved-by-logic: validate resolvedBy logic",
  // The `output.validate` is not supported
  "rollup@function@validate-output: handles validate failure",

  // Module meta related
  // Shouldn't modify meta objects passed in resolveId hook
  "rollup@function@reuse-resolve-meta: does not modify meta objects passed in resolveId",
  "rollup@function@modify-meta: allows to freely modify moduleInfo.meta and maintain object identity",
  "rollup@function@custom-module-options: supports adding custom options to modules",
  "rollup@function@custom-external-module-options: supports adding custom options to external modules",

  // The `output.file` is not supported
  "rollup@function@file-and-dir: throws when using both the file and the dir option",

  // The `input.perf` and `bundle.getTimings()` is not supported
  "rollup@function@adds-timings-to-bundle-when-codesplitting: Adds timing information to bundle when bundling with perf=true",
  "rollup@function@adds-timings-to-bundle: Adds timing information to bundle when bundling with perf=true",

  // The `input.context` is not supported
  "rollup@form@custom-context: allows custom context@generates es",

  // The `input.moduleContext` is not supported
  "rollup@form@custom-module-context-function: allows custom module-specific context with a function option",
  "rollup@form@custom-module-context: allows custom module-specific context@generates es",

  // The `output.paths` is not supported
  "rollup@function@re-export-own: avoid using export.hasOwnProperty",
  "rollup@function@mixed-external-paths: allows using the path option selectively",
  "rollup@form@paths-function: external paths (#754)@generates es",
  "rollup@form@paths-relative: external paths (#754)@generates es",
  "rollup@form@paths: external paths (#754)@generates es",
  // The `output.compact` is not supported
  "rollup@function@inlined-dynamic-namespace-compact: properly resolves inlined dynamic namespaces in compact mode",
  "rollup@function@compact: compact output with compact: true", // Check test runner
  "rollup@form@compact-multiple-imports: correctly handles empty external imports in compact mode@generates es",
  "rollup@form@compact: supports compact output with compact: true@generates es",

  // The `import.meta.url` is not supported
  "rollup@function@import-meta-url-b: Access document.currentScript at the top level",
  "rollup@function@import-meta-url: resolves import.meta.url",
  "rollup@form@import-meta-url: supports import.meta.url@generates es",
  "rollup@form@resolve-import-meta-url-export: correctly exports resolved import.meta.url@generates es",
  "rollup@form@resolve-import-meta-url: allows to configure import.meta.url@generates es",

  // output.format systemjs is not supported
  "rollup@form@system-comments: Correctly places leading comments when rendering system bindings",
  "rollup@form@system-default-comments: Correctly places leading comments when rendering system default exports",
  "rollup@form@system-export-declarations: Renders declarations where some variables are exported",
  "rollup@form@system-export-destructuring-declaration: supports destructuring declarations for systemJS",
  "rollup@form@system-export-rendering-compact: Renders updates of exported variables for SystemJS output in compact mode",
  "rollup@form@system-export-rendering: Renders updates of exported variables for SystemJS output",
  "rollup@form@system-module-reserved: does not output reserved system format identifiers",
  "rollup@form@system-multiple-export-bindings: supports multiple live bindings for the same symbol in systemJS",
  "rollup@form@system-null-setters: allows to avoid null setters for side effect only imports",
  "rollup@form@system-reexports: merges reexports in systemjs",
  "rollup@form@system-semicolon: supports asi in system binding output",
  "rollup@form@system-uninitialized: supports uninitialized binding exports",
  "rollup@form@import-namespace-systemjs: imports namespace (systemjs only)",
  "rollup@form@modify-export-semi: inserts semicolons correctly when modifying SystemJS exports@generates system",

  // Should delete use strict from function body
  "rollup@function@function-use-strict-directive-removed: should delete use strict from function body",

  // The module information is not compatible with rollup
  "rollup@function@plugin-module-information-no-cache: handles accessing module information via plugins with cache disabled",
  "rollup@function@plugin-module-information: provides module information on the plugin context",
  "rollup@function@module-parsed-hook: calls the moduleParsedHook once a module is parsed",
  "rollup@function@has-default-export: reports if a module has a default export", // hasDefaultExport is not support
  "rollup@function@context-resolve: returns the correct results for the context resolve helper",
  "rollup@function@check-exports-exportedBindings-as-a-supplementary-test: check exports and exportedBindings in moduleParsed as a supplementary test",
  "rollup@function@load-resolve-dependencies: allows to wait for dependency resolution in this.load to scan dependency trees", // importedIdResolutions

  // The chunk information is not compatible with rollup
  "rollup@form@addon-functions: provides module information when adding addons@generates es",
  
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

  // The namespace object is not compatible with rollup
  "rollup@function@namespaces-have-null-prototype: creates namespaces with null prototypes",
  "rollup@function@namespaces-are-frozen: namespaces should be non-extensible and its properties immutatable and non-configurable",
  "rollup@function@namespace-override: does not warn when overriding namespace reexports with explicit ones",
  "rollup@function@keep-cjs-dynamic-import: keeps dynamic imports in CJS output by default",
  "rollup@function@escape-arguments: does not use \"arguments\" as a placeholder variable for a default export",
  "rollup@function@dynamic-import-only-default: correctly imports dynamic namespaces with only a default export from entry- and non-entry-point chunks",
  "rollup@function@dynamic-import-default-mode-facade: handles dynamic imports from facades using default export mode",
  "rollup@function@chunking-duplicate-reexport: handles duplicate reexports when using dynamic imports",

  // Passed, but the output snapshot is same as rollup
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


  // Should give error or warinings
  // The output.generatedCode.preset is not supported 
  "rollup@function@unknown-generated-code-preset: throws for unknown presets for the generatedCode option",
  // The output.generatedCode is not supported 
  "rollup@function@unknown-generated-code-value: throws for unknown string values for the generatedCode option",
  // The output.treeshake.preset is not supported 
  "rollup@function@unknown-treeshake-preset: throws for unknown presets for the treeshake option",  
  // Give warning if return map or ast without code
  "rollup@function@transform-without-code-warn-ast: warns when returning a map but no code from a transform hook",
  "rollup@function@transform-without-code-warn-map: warns when returning a map but no code from a transform hook",
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
  // Throw error for invalid option at load hook
  "rollup@function@load-returns-string-or-null: throws error if load returns something wacky",    
  // Give warning for empty chunk
  "rollup@function@vars-with-init-in-dead-branch: handles vars with init in dead branch (#1198)",
  // Give parse error for update imported bindings
  "rollup@function@update-expression-of-import-fails: disallows updates to imported bindings",
  "rollup@function@reassign-import-not-at-top-level-fails: disallows assignments to imported bindings not at the top level",
  "rollup@function@reassign-import-fails: disallows assignments to imported bindings",
  // Give warning for unused imports
  "rollup@function@unused-import: warns on unused imports ([#595])",
  // Give warns when input hooks are used in output plugins
  "rollup@function@per-output-plugins-warn-hooks: warns when input hooks are used in output plugins",
  // Give warning for module level directive
  "rollup@function@module-level-directive: module level directives should produce warnings",    
  // Give parse error for non-top-level imports
  "rollup@function@import-not-at-top-level-fails: disallows non-top-level imports",
  // Give parse error for non-top-level exports
  "rollup@function@export-not-at-top-level-fails: disallows non-top-level exports",
  // Give error for invalid hash length
  "rollup@function@hashing@maximum-hash-size: throws when the maximum hash size is exceeded",
  "rollup@function@hashing@minimum-hash-size: throws when the maximum hash size is exceeded",
  // Give error for placeholder length for non-hash placeholder
  "rollup@function@hashing@length-at-non-hash: throws when configuring a length for placeholder other than \"hash\"",
  // Give error for invalid emit file type
  "rollup@function@emit-file@invalid-file-type: throws for invalid file types",
  // Give error for invalid asset name
  "rollup@function@emit-file@invalid-asset-name3: throws for invalid asset names with absolute path on Windows OS",
  "rollup@function@emit-file@invalid-asset-name: throws for invalid asset names",
  // Give warns if multiple files with the same name are emitted
  "rollup@function@emit-file@emit-same-file: warns if multiple files with the same name are emitted",
  "rollup@function@emit-file@emit-from-output-options: throws when trying to emit files from the outputOptions hook",
  "rollup@function@duplicate-import-specifier-fails: disallows duplicate import specifiers",
  "rollup@function@duplicate-import-fails: disallows duplicate imports",
  "rollup@function@double-named-export: throws on duplicate named exports",
  "rollup@function@double-named-reexport: throws on duplicate named exports",
  "rollup@function@double-default-export: throws on double default exports",
  "rollup@function@deprecations@externalImportAssertions: marks the \"output.externalImportAssertions\" option as deprecated",
  "rollup@function@cannot-call-external-namespace: warns if code calls an external namespace",
  "rollup@function@cannot-call-internal-namespace: warns if code calls an internal namespace",
  "rollup@function@circular-reexport: throws proper error for circular reexports",
  "rollup@function@conflicting-reexports@namespace-import: warns when a conflicting binding is imported via a namespace import", 
  "rollup@function@cannot-resolve-sourcemap-warning: handles when a sourcemap cannot be resolved in a warning",
  "rollup@function@adds-json-hint-for-missing-export-if-is-json-file: should provide json hint when importing a no export json file",
  "rollup@function@add-watch-file-generate: throws when adding watch files during generate",

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
