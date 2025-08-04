# Tests failed by unsupported features

## Plugin related

### The `rollup.rollup` api is not compatible with rollup, the build is start at `bundle.generate` or `bundle.write`, so the input plugin hooks is not called
 - rollup@hooks@supports buildStart and buildEnd hooks
 - rollup@hooks@supports warnings in buildStart and buildEnd hooks
 - rollup@hooks@passes errors to the buildEnd hook

### The `NormalziedOptions` at hooks is not compatible with rollup
 - rollup@function@options-hook: allows to read and modify options in the options hook
 - rollup@function@output-options-hook: allows to read and modify options in the options hook

### The `load` hook return `ast` is not supported
 - rollup@function@uses-supplied-ast: uses supplied AST

### The `resolveId` hook `resolvedBy` is not supported
 - rollup@function@validate-resolved-by-logic: validate resolvedBy logic

### The `shouldTransformCachedModule` hook is not supported
 - rollup@function@plugin-error-should-transform: errors in shouldTransformCachedModule abort the build

### The `resolveDynamicImport` hook `specifier: AstNode` not supported
 - rollup@form@dynamic-import-unresolvable: Returns the raw AST nodes for unresolvable dynamic imports@generates es

### The plugin `sequential` is not supported
 - rollup@function@enforce-sequential-plugin-order: allows to enforce sequential plugin hook order for parallel plugin hooks
 - rollup@hooks@allows to enforce sequential plugin hook order in watch mode

### The `renderDynamicImport/resolveFileUrl/resolveImportMeta/shouldTransformCachedModule` hooks not supported
 - rollup@function@enforce-plugin-order: allows to enforce plugin hook order
 
### The `renderDynamicImport` hook not supported
 - rollup@form@custom-dynamic-import-no-interop: does not add any interop when using a custom dynamic import handler@generates es

### The `resolveFileUrl` hook not supported
 - rollup@form@configure-file-url: allows to configure file urls@generates es

### The `PluginContext.parse` does not support `allowReturnOutsideFunction` option
 - rollup@function@parse-return-outside-function: supports parsing return statements outside functions via options

### The `PluginContext.cache` is not supported
 - rollup@function@plugin-cache@anonymous-delete: throws for anonymous plugins deleting from the cache
 - rollup@function@plugin-cache@anonymous-get: throws for anonymous plugins reading the cache
 - rollup@function@plugin-cache@anonymous-has: throws for anonymous plugins checking the cache
 - rollup@function@plugin-cache@anonymous-set: throws for anonymous plugins adding to the cache
 - rollup@function@plugin-cache@duplicate-names: throws if two plugins with the same name and no cache key access the cache
 - rollup@hooks@Disables the default transform cache when using cache in transform only
 - rollup@hooks@opts-out transform hook cache for custom cache

### The `PluginContext.load` is not fully supported
 - rollup@form@supports-core-js: supports core-js (`@rollup/plugin-commonjs` is not supported)
 - rollup@form@supports-es5-shim: supports es5-shim (`@rollup/plugin-commonjs` is not supported)
 - rollup@form@supports-es6-shim: supports es6-shim (`@rollup/plugin-commonjs` is not supported)
 - rollup@function@preload-cyclic-module: handles pre-loading a cyclic module in the resolveId hook (load entry module at resolveId hook)
 - rollup@function@preload-module: allows pre-loading modules via this.load (load entry module at resolveId hook)
 - rollup@function@module-side-effects@writable: ModuleInfo.moduleSideEffects should be writable during build time(load entry module at resolveId hook)
 - rollup@function@modify-meta: allows to freely modify moduleInfo.meta and maintain object identity

### The `maxParallelFileOps` is not supported
 - rollup@function@max-parallel-file-operations@default: maxParallelFileOps not set
 - rollup@function@max-parallel-file-operations@error: maxParallelFileOps: fileRead error is forwarded
 - rollup@function@max-parallel-file-operations@infinity: maxParallelFileOps set to infinity
 - rollup@function@max-parallel-file-operations@set: maxParallelFileOps set to 3
 - rollup@function@max-parallel-file-operations@with-plugin: maxParallelFileOps with plugin

### The `PluginContext.emitFile` emit chunk is only supported partially
 - rollup@function@emit-chunk-hash: gives access to the hashed filed name via this.getFileName in generateBundle
 - rollup@function@implicit-dependencies@dependant-dynamic-import-no-effects: throws when a module that is loaded before an emitted chunk is fully tree-shaken
 - rollup@function@implicit-dependencies@dependant-dynamic-import-not-included: throws when a module that is loaded before an emitted chunk is only linked to the module graph via a tree-shaken dynamic import
 - rollup@function@implicit-dependencies@dependant-not-part-of-graph: throws when a module that is loaded before an emitted chunk is not part of the module graph
 - rollup@function@implicit-dependencies@external-dependant: throws when a module that is loaded before an emitted chunk does not exist
 - rollup@function@implicit-dependencies@missing-dependant: throws when a module that is loaded before an emitted chunk is external
 - rollup@function@emit-file@set-asset-source-chunk: throws when trying to set the asset source of a chunk
 - rollup@function@emit-file@modules-loaded: Throws when adding a chunk after the modules have finished loading
 - rollup@function@emit-file@invalid-chunk-id: throws for invalid chunk ids
 - rollup@function@emit-file@chunk-not-found: Throws if an emitted entry chunk cannot be resolved
 - rollup@function@emit-file@chunk-filename-not-available-buildEnd: Throws when accessing the filename before it has been generated in buildEnd
 - rollup@function@emit-file@chunk-filename-not-available-renderStart: Throws when accessing the filename before it has been generated in renderStart
 - rollup@function@emit-file@chunk-filename-not-available: Throws when accessing the filename before it has been generated
 - rollup@function@emit-file@file-references-in-bundle: lists referenced files in the bundle
 - rollup@hooks@caches chunk emission in transform hook

### The `PluginContext.emitFile` emit prebuilt chunk is not supported 
 - rollup@function@emit-file@prebuilt-chunk: get right prebuilt chunks
 - rollup@function@emit-file@invalid-prebuilt-chunk-filename: throws for invalid prebuilt chunks filename
 - rollup@function@emit-file@invalid-prebuit-chunk-code: throws for invalid prebuilt chunks code

### The `PluginContext.setAssetSource` is not supported
 - rollup@function@emit-file@asset-source-invalid2: throws when setting an empty asset source
 - rollup@function@emit-file@asset-source-invalid3: throws when setting an empty asset source
 - rollup@function@emit-file@asset-source-invalid4: throws when setting an empty asset source
 - rollup@function@emit-file@set-asset-source-transform: throws when setting the asset source in the transform hook
 - rollup@function@emit-file@set-source-in-output-options: throws when trying to set file sources in  the outputOptions hook
 - rollup@function@emit-file@set-asset-source-twice2: throws when setting the asset source twice
 - rollup@function@emit-file@set-asset-source-twice: throws when setting the asset source twice
 - rollup@function@emit-file@invalid-set-asset-source-id: throws for invalid asset ids
 - rollup@hooks@keeps emitted ids stable between runs

### The `PluginContext.error` accpet more arguments at `transform` hooks 
 - rollup@function@plugin-error-transform-pos: `this.error(...)` accepts number as second parameter (#5044)
 - rollup@function@plugin-error-loc-instead-pos: `this.error(...)` accepts { line, column } object as second parameter (#1265)
 
## Options related

### The `output.format` systemjs is not supported
 - rollup@form@system-comments: Correctly places leading comments when rendering system bindings
 - rollup@form@system-default-comments: Correctly places leading comments when rendering system default exports
 - rollup@form@system-export-declarations: Renders declarations where some variables are exported
 - rollup@form@system-export-destructuring-declaration: supports destructuring declarations for systemJS
 - rollup@form@system-export-rendering-compact: Renders updates of exported variables for SystemJS output in compact mode
 - rollup@form@system-export-rendering: Renders updates of exported variables for SystemJS output
 - rollup@form@system-module-reserved: does not output reserved system format identifiers
 - rollup@form@system-multiple-export-bindings: supports multiple live bindings for the same symbol in systemJS
 - rollup@form@system-null-setters: allows to avoid null setters for side effect only imports
 - rollup@form@system-reexports: merges reexports in systemjs
 - rollup@form@system-semicolon: supports asi in system binding output
 - rollup@form@system-uninitialized: supports uninitialized binding exports
 - rollup@form@import-namespace-systemjs: imports namespace (systemjs only)
 - rollup@form@modify-export-semi: inserts semicolons correctly when modifying SystemJS exports@generates system

### The `input.perf` and `bundle.getTimings()` is not supported
 - rollup@function@adds-timings-to-bundle-when-codesplitting: Adds timing information to bundle when bundling with perf=true
 - rollup@function@adds-timings-to-bundle: Adds timing information to bundle when bundling with perf=true

### The `input.context` is not supported
 - rollup@form@custom-context: allows custom context@generates es
 - rollup@function@options-in-renderstart: makes input and output options available in renderStart

### `output.dynamicImportInCjs` is not supported
 - rollup@function@dynamic-import-this-function: uses correct "this" in dynamic imports when not using arrow functions
 - rollup@function@dynamic-import-this-arrow: uses correct "this" in dynamic imports when using arrow functions
 - rollup@function@dynamic-import-expression: Dynamic import expression replacement
 - rollup@function@external-dynamic-import-live-binding-compact: supports external dynamic imports with live bindings in compact mode
 - rollup@function@external-dynamic-import-live-binding: supports external dynamic imports with live bindings
 - rollup@function@no-external-live-bindings: Allows omitting the code that handles external live bindings
 - rollup@function@no-external-live-bindings-compact: Allows omitting the code that handles external live bindings
  
### The `input.moduleContext` is not supported
 - rollup@form@custom-module-context-function: allows custom module-specific context with a function option
 - rollup@form@custom-module-context: allows custom module-specific context@generates es

### The `output.paths` is not supported
 - rollup@function@re-export-own: avoid using export.hasOwnProperty
 - rollup@function@mixed-external-paths: allows using the path option selectively
 - rollup@form@paths-function: external paths (#754)@generates es
 - rollup@form@paths-relative: external paths (#754)@generates es
 - rollup@form@paths: external paths (#754)@generates es

### The `output.compact` is not supported
 - rollup@function@inlined-dynamic-namespace-compact: properly resolves inlined dynamic namespaces in compact mode
 - rollup@function@compact: compact output with compact: true
 - rollup@form@compact-multiple-imports: correctly handles empty external imports in compact mode@generates es
 - rollup@form@compact: supports compact output with compact: true@generates es

### The `output.validate` is not supported
 - rollup@function@validate-output: handles validate failure

### The `output.interop` is not supported
 - rollup@function@interop-auto-live-bindings: handles interop "auto" with live-bindings support
 - rollup@function@interop-auto-no-live-bindings: handles interop "auto" without live-bindings support
 - rollup@function@interop-default-conflict: handles conflicts with added interop default variables and supports default live bindings
 - rollup@function@interop-default-only-named-import: throws when using a named import with interop "defaultOnly"
 - rollup@function@interop-default-only-named-namespace-reexport: allows reexporting a namespace as a name when interop is "defaultOnly"
 - rollup@function@interop-default-only-named-reexport: throws when reexporting a namespace with interop "defaultOnly"
 - rollup@function@interop-default-only-namespace-import: allows importing a namespace when interop is "defaultOnly"
 - rollup@function@interop-default-only-namespace-reexport: warns when reexporting a namespace with interop "defaultOnly"
 - rollup@function@interop-default-only: handles interop "defaultOnly"
 - rollup@function@interop-default: handles interop "default" with live-bindings support
 - rollup@function@interop-esmodule: handles interop "esModule" with live-bindings support
 - rollup@function@invalid-interop: throws for invalid interop values
 - rollup@function@deconflicts-interop: deconflicts the interop function
 - rollup@form@interop-per-dependency-no-live-binding: allows to configure the interop type per external dependency
 - rollup@form@interop-per-dependency: allows to configure the interop type per external dependency@generates es
 - rollup@form@interop-per-reexported-dependency: allows to configure the interop type per reexported external dependency@generates es

### The `Bundle.cache` is not supported
 - rollup@function@module-tree: bundle.modules includes dependencies (#903)
 - rollup@function@has-modules-array: user-facing bundle has modules array

### The `output.generatedCode` is not supported 
 - rollup@form@generated-code-compact@arrow-functions-false: does not use arrow functions@generates es
 - rollup@form@generated-code-compact@arrow-functions-true: uses arrow functions@generates es
 - rollup@form@generated-code-compact@const-bindings-false: does not use block bindings@generates es
 - rollup@form@generated-code-compact@const-bindings-true: uses block bindings@generates es
 - rollup@form@generated-code-compact@object-shorthand-false: does not use object shorthand syntax
 - rollup@form@generated-code-compact@object-shorthand-true: uses object shorthand syntax
 - rollup@form@generated-code-compact@reserved-names-as-props-false: escapes reserved names used as props@generates es
 - rollup@form@generated-code-compact@reserved-names-as-props-true: escapes reserved names used as props@generates es
 - rollup@form@generated-code-presets@es2015: handles generatedCode preset "es2015"
 - rollup@form@generated-code-presets@es5: handles generatedCode preset "es5"
 - rollup@form@generated-code-presets@preset-with-override: handles generatedCode preset "es2015"
 - rollup@form@generated-code@arrow-functions-false: does not use arrow functions@generates es
 - rollup@form@generated-code@arrow-functions-true: uses arrow functions@generates es
 - rollup@form@generated-code@const-bindings-false: does not use block bindings@generates es
 - rollup@form@generated-code@const-bindings-true: uses block bindings@generates es
 - rollup@form@generated-code@object-shorthand-false: does not use object shorthand syntax
 - rollup@form@generated-code@object-shorthand-true: uses object shorthand syntax
 - rollup@form@generated-code@reserved-names-as-props-false: escapes reserved names used as props@generates es
 - rollup@form@generated-code@reserved-names-as-props-true: escapes reserved names used as props@generates es

### The `output.generatedCode.symbols` is not supported 
 - rollup@function@reexport-ns: external namespace reexport
 - rollup@function@namespace-tostring@dynamic-import-default-mode: adds Symbol.toStringTag property to dynamic imports of entry chunks with default export mode
 - rollup@function@namespace-tostring@dynamic-import: adds Symbol.toStringTag property to dynamic imports
 - rollup@function@namespace-tostring@entry-named: adds Symbol.toStringTag property to entry chunks with named exports
 - rollup@function@namespace-tostring@external-namespaces: adds Symbol.toStringTag property to external namespaces
 - rollup@function@namespace-tostring@inlined-namespace: adds Symbol.toStringTag property to inlined namespaces
 - rollup@function@namespace-tostring@interop-property-descriptor: generated interop namespaces should have correct Symbol.toStringTag
 - rollup@function@namespace-tostring@property-descriptor: namespace export should have @@toStringTag with correct property descriptors #4336
 - rollup@function@name-conflict-symbol: avoids name conflicts with local variables named Symbol
 - rollup@form@namespace-tostring@inlined-namespace-static-resolution: statically resolves Symbol.toStringTag for inlined namespaces
 - rollup@form@namespace-tostring@inlined-namespace: adds Symbol.toStringTag property to inlined namespaces@generates es

### The `output.preserveModules` is not compatible yet
 - rollup@function@preserve-modules-default-mode-namespace: import namespace from chunks with default export mode when preserving modules,
 - rollup@function@circular-preserve-modules: correctly handles circular dependencies when preserving modules
 - rollup@function@missing-export-preserve-modules: supports shimming missing exports when preserving modules
 - rollup@function@preserve-modules-circular-order: preserves execution order for circular dependencies when preserving modules
 - rollup@function@preserve-modules@inline-dynamic-imports: Inlining dynamic imports is not supported when preserving modules
 - rollup@function@preserve-modules@invalid-default-export-mode: throws when using default export mode with named exports
 - rollup@function@preserve-modules@invalid-no-preserve-entry-signatures: throws when setting preserveEntrySignatures to false
 - rollup@function@preserve-modules@invalid-none-export-mode: throws when using none export mode with named exports
 - rollup@function@preserve-modules@manual-chunks: Assigning manual chunks fails when preserving modules
 - rollup@function@preserve-modules@mixed-exports: warns for mixed exports in all chunks when preserving modules
 - rollup@function@preserve-modules@virtual-modules-conflict: Generates actual files for virtual modules when preserving modules
 - rollup@function@preserve-modules@virtual-modules: Generates actual files for virtual modules when preserving modules
 - rollup@function@synthetic-named-exports@preserve-modules: handles a dynamic import with synthetic named exports in preserveModules mode
 - rollup@function@circular-namespace-reexport-preserve-modules: correctly handles namespace reexports with circular dependencies when preserving modules

### The `output.manualChunks` is not compatible
 - rollup@function@manual-chunks-conflict: Throws for conflicts between manual chunks
 - rollup@function@manual-chunks-include-external-modules3: throws an error EXTERNAL_MODULES_CANNOT_BE_TRANSFORMED_TO_MODULES for manualChunks' modules that are resolved as an external module by the 'external' option
 - rollup@function@manual-chunks-include-external-modules: throws for manualChunks' modules that are resolved as an external module by plugins
 - rollup@function@manual-chunks-info: provides additional chunk information to a manualChunks function
 - rollup@function@circular-namespace-reexport-manual-chunks: correctly handles namespace reexports with circular dependencies when using manual chunks
 - rollup@function@emit-chunk-manual-asset-source: supports setting asset sources as side effect of the manual chunks option
 - rollup@function@emit-chunk-manual: supports emitting chunks as side effect of the manual chunks option
 - rollup@function@inline-imports-with-manual: Manual chunks are not supported when inlining dynamic imports

### The `format: amd` not supported
 - rollup@function@amd-auto-id-id: throws when using both the amd.autoId and the amd.id option
 - rollup@function@amd-base-path-id: throws when using both the amd.basePath and the amd.id option
 - rollup@function@amd-base-path: throws when using only amd.basePath option

### The `output.sourcemapExcludeSources` is not supported
 - rollup@form@sourcemaps-excludesources: correct sourcemaps are written (excluding sourceContent)@generates es

### The `output.sourcemapBaseUrl` is not compatible yet
 - rollup@function@sourcemap-base-url-invalid: throws for invalid sourcemapBaseUrl
 - rollup@sourcemaps@sourcemap-base-url-without-trailing-slash: add a trailing slash automatically if it is missing@generates es
 - rollup@sourcemaps@sourcemap-base-url: adds a sourcemap base url@generates es

### The `output.generatedCode.preset` is not supported 
 - rollup@function@unknown-generated-code-preset: throws for unknown presets for the generatedCode option

### The `output.generatedCode` is not supported
 - rollup@function@unknown-generated-code-value: throws for unknown string values for the generatedCode option

### The `output.treeshake.preset` is not supported 
 - rollup@function@unknown-treeshake-preset: throws for unknown presets for the treeshake option

### The `output.treeshake.moduleSideEffect` is not compatible with rollup
 - rollup@function@module-side-effects@resolve-id-external: does not include modules without used exports if moduleSideEffect is false
 - rollup@function@module-side-effects@resolve-id: does not include modules without used exports if moduleSideEffect is false

### The `ModuleInfo` is not compatible with rollup
 - rollup@function@plugin-module-information-no-cache: handles accessing module information via plugins with cache disabled
 - rollup@function@plugin-module-information: provides module information on the plugin context
 - rollup@function@module-parsed-hook: calls the moduleParsedHook once a module is parsed
 - rollup@function@has-default-export: reports if a module has a default export (`hasDefaultExport`)
 - rollup@function@context-resolve: returns the correct results for the context resolve helper
 - rollup@function@check-exports-exportedBindings-as-a-supplementary-test: check exports and exportedBindings in moduleParsed as a supplementary test
 - rollup@function@load-resolve-dependencies: allows to wait for dependency resolution in this.load to scan dependency trees (`importedIdResolutions`) 
 - rollup@function@resolve-relative-external-id: resolves relative external ids

### The chunk information is not compatible with rollup
 - rollup@form@addon-functions: provides module information when adding addons@generates es
 - rollup@hooks@supports generateBundle hook including reporting rendered exports and source length(`modules.dep.renderedExports/removedExports`)

### The rolldown `output.dir` default to be `dist`, the rollup not specific `dir` or `file` by default
 - rollup@hooks@Throws when not specifying "file" or "dir"

## Features

### The `import.meta.ROLLUP_FILE_URL_<referenceId>` is not supported
 - rollup@form@emit-asset-file: supports emitting assets from plugin hooks@generates es
 - rollup@form@emit-uint8array-no-buffer: supports emitting assets as Uint8Arrays when Buffer is not available@generates es
 - rollup@hooks@caches asset emission in transform hook

### The rollup treat non-js-extensions module as js module, but the rolldown wiill guess the module type by externsion
 - rollup@function@non-js-extensions: non .js extensions are preserved

### The `syntheticNamedExports` is not supported
 - rollup@form@synthetic-named-exports: synthetic named exports
 - rollup@function@synthetic-named-exports-fallback-es2015: adds a fallback in case synthetic named exports are falsy
 - rollup@function@synthetic-named-exports-fallback: adds a fallback in case synthetic named exports are falsy
 - rollup@function@synthetic-named-exports@circular-synthetic-exports2: handles circular synthetic exports
 - rollup@function@synthetic-named-exports@circular-synthetic-exports: handles circular synthetic exports
 - rollup@function@synthetic-named-exports@dynamic-import: supports dynamically importing a module with synthetic named exports
 - rollup@function@synthetic-named-exports@entry: does not expose the synthetic namespace if an entry point uses a string value
 - rollup@function@synthetic-named-exports@external-synthetic-exports: external modules can not have syntheticNamedExports
 - rollup@function@synthetic-named-exports@namespace-object: does not include named synthetic namespaces in namespace objects
 - rollup@function@synthetic-named-exports@namespace-overrides: supports re-exported synthetic exports in namespace objects with correct export precedence
 - rollup@function@synthetic-named-exports@non-default-export: supports providing a named export to generate synthetic exports
 - rollup@function@synthetic-named-exports@synthetic-exports-need-default: synthetic named exports modules need a default export
 - rollup@function@synthetic-named-exports@synthetic-exports-need-fallback-export: synthetic named exports modules need their fallback export
 - rollup@function@synthetic-named-exports@synthetic-named-export-as-default: makes sure default exports of synthetic named exports are snapshots
 - rollup@function@synthetic-named-exports@synthetic-named-export-entry: does not expose synthetic named exports on entry points
 - rollup@function@reexport-from-synthetic: handles reexporting a synthetic namespace from a non-synthetic module
 - rollup@function@respect-synthetic-export-reexporter-side-effects: respect side-effects in reexporting modules even if moduleSideEffects are off
 - rollup@function@internal-reexports-from-external: supports namespaces with external star reexports
 - rollup@function@deconflict-synthetic-named-export-cross-chunk: deconflicts synthetic named exports across chunks
 - rollup@function@deconflict-synthetic-named-export: deconflicts synthetic named exports
 - rollup@form@entry-with-unused-synthetic-exports: does not include unused synthetic namespace object in entry points@generates es
 - rollup@form@merge-namespaces-non-live: merges namespaces without live-bindings
 - rollup@form@merge-namespaces: merges namespaces with live-bindings

### The `import.meta.url` is not compatible
 - rollup@function@import-meta-url-b: Access document.currentScript at the top level
 - rollup@form@import-meta-url: supports import.meta.url@generates es
 - rollup@form@resolve-import-meta-url-export: correctly exports resolved import.meta.url@generates es
 - rollup@form@resolve-import-meta-url: allows to configure import.meta.url@generates es

### Import Assertions is not supported
 - rollup@function@import-assertions@plugin-assertions-this-resolve: allows plugins to provide assertions for this.resolve'
 - rollup@function@import-assertions@plugin-assertions-this-resolve: allows plugins to provide attributes for this.resolve
 - rollup@function@import-assertions@warn-assertion-conflicts: warns for conflicting import attributes
 - rollup@function@import-assertions@warn-unresolvable-assertions: warns for dynamic import attributes that cannot be resolved
 - rollup@form@deprecated@removes-dynamic-assertions: keep import assertions for dynamic imports
 - rollup@form@deprecated@removes-static-attributes: keeps any import assertions on input
  
### Import attributes is not supported
 - rollup@form@import-attributes@attribute-shapes: handles special shapes of attributes
 - rollup@form@import-attributes@keep-dynamic-assertions: keep import attributes for dynamic imports@generates es
 - rollup@form@import-attributes@keep-dynamic-attributes: keep import attributes for dynamic imports@generates es
 - rollup@form@import-attributes@keeps-static-assertions: keeps any import assertions on input@generates es
 - rollup@form@import-attributes@keeps-static-attributes: keeps any import attributes on input@generates es
 - rollup@form@import-attributes@plugin-attributes-resolvedynamicimport: allows plugins to read and write import attributes in resolveDynamicImport
 - rollup@form@import-attributes@plugin-attributes-resolveid: allows plugins to read and write import attributes in resolveId
 - rollup@form@import-attributes@removes-dynamic-attributes: keep import attributes for dynamic imports
 - rollup@form@import-attributes@removes-static-attributes: keeps any import attributes on input

### watch behavior is not compatible yet
 - rollup@hooks@allows to enforce plugin hook order in watch mode

### escaping external id is not supported
 - rollup@form@quote-id: handles escaping for external ids@generates es

### remove `use strict` from function body
 - rollup@function@function-use-strict-directive-removed: should delete use strict from function body

### comment related
 - rollup@form@comment-before-import: preserves comments before imports@generates es
 - rollup@form@comment-start-inside-comment: properly remove coments above import statements@generates es

### The namespace object is not compatible with rollup
 - rollup@function@namespaces-have-null-prototype: creates namespaces with null prototypes
 - rollup@function@namespaces-are-frozen: namespaces should be non-extensible and its properties immutatable and non-configurable
 - rollup@function@namespace-override: does not warn when overriding namespace reexports with explicit ones
 - rollup@function@keep-cjs-dynamic-import: keeps dynamic imports in CJS output by default
 - rollup@function@escape-arguments: does not use "arguments" as a placeholder variable for a default export
 - rollup@function@dynamic-import-only-default: correctly imports dynamic namespaces with only a default export from entry- and non-entry-point chunks
 - rollup@function@dynamic-import-default-mode-facade: handles dynamic imports from facades using default export mode
 - rollup@function@chunking-duplicate-reexport: handles duplicate reexports when using dynamic imports

### Rewrite top-level `this` to `undefined`
 - rollup@form@proper-this-context: make sure "this" respects the context for arrow functions
 - rollup@form@this-is-undefined: top-level `this` expression is rewritten as `undefined`@generates es
 - rollup@function@warn-on-top-level-this: warns on top-level this (#770)

### The error/warning information is not compatible with rollup
 - rollup@function@warn-on-eval: warns about use of eval
 - rollup@function@warn-missing-iife-name: warns if no name is provided for an IIFE bundle
 - rollup@function@plugin-error-with-numeric-code: rollup do not break if get a plugin error that contains numeric code
 - rollup@function@warn-on-auto-named-default-exports: warns if default and named exports are used in auto mode
 - rollup@function@namespace-reassign-import-fails: warns for reassignments to namespace exports
 - rollup@function@namespace-update-import-fails: disallows updates to namespace exports
 - rollup@function@load-module-error@load: throws when a module cannot be loaded
 - rollup@function@external-entry-point: throws for entry points that are resolved as false by plugins
 - rollup@function@external-entry-point-object: throws for entry points that are resolved as an external object by plugins
 - rollup@function@export-type-mismatch-c: cannot have named exports if explicit export type is default
 - rollup@function@export-type-mismatch: cannot have named exports if explicit export type is default
 - rollup@function@error-missing-umd-name: throws an error if no name is provided for a UMD bundle
 - rollup@function@dynamic-import-not-found: warns if a dynamic import is not found
 - rollup@function@does-not-hang-on-missing-module: does not hang on missing module (#53)
 - rollup@function@default-not-reexported: default export is not re-exported with export *
 - rollup@function@banner-and-footer: adds a banner/footer
 - rollup@function@check-resolve-for-entry: checks that entry is resolved
 - rollup@function@custom-path-resolver-plural-b: resolver error is not caught
 - rollup@function@conflicting-reexports@named-import: throws when a conflicting binding is imported via a named import
 - rollup@function@file-and-dir: throws when using both the file and the dir option
 - rollup@function@reexport-missing-error: reexporting a missing identifier should print an error
 - rollup@function@load-module-error@transform: plugin transform hooks can use `this.error({...}, char)` (#1140)
 - rollup@function@plugin-error@transform: plugin transform hooks can use `this.error({...}, char)` (#1140)
  - rollup@function@plugin-error@buildEnd: buildStart hooks can use this.error
 - rollup@function@plugin-error@buildStart: buildStart hooks can use this.error
 - rollup@function@plugin-error@generateBundle: buildStart hooks can use this.error
 - rollup@function@plugin-error@load: buildStart hooks can use this.error
 - rollup@function@plugin-error@renderChunk: buildStart hooks can use this.error
 - rollup@function@plugin-error@renderStart: buildStart hooks can use this.error
 - rollup@function@plugin-error@resolveId: buildStart hooks can use this.error
 - rollup@function@load-module-error@buildEnd: buildStart hooks can use this.error
 - rollup@function@load-module-error@buildStart: buildStart hooks can use this.error
 - rollup@function@load-module-error@generateBundle: buildStart hooks can use this.error
 - rollup@function@load-module-error@renderChunk: buildStart hooks can use this.error
 - rollup@function@load-module-error@renderStart: buildStart hooks can use this.error
 - rollup@function@load-module-error@resolveId: buildStart hooks can use this.error
 - rollup@function@logging@this-error-onlog: can turn logs into errors via this.error in the onLog hook
 - rollup@function@plugin-error-only-first-render-chunk: throws error only with first plugin renderChunk
 - rollup@function@plugin-error-only-first-transform: throws error only with first plugin transform
 - rollup@function@plugin-error-module-parsed: errors in moduleParsed abort the build
 - rollup@function@module-side-effects@external-false: supports setting module side effects to false for external modules
 - rollup@function@logging@handle-logs-in-plugins: allows plugins to read and filter logs
 - rollup@function@logging@promote-log-to-error: allows turning logs into errors
 - rollup@hooks@Throws when using the "file"" option for multiple chunks
 - rollup@hooks@supports renderError hook

### The error/warning not implement
 - rollup@hooks@Throws when using the "sourcemapFile" option for multiple chunks
 - rollup@function@transform-without-sourcemap-render-chunk: preserves sourcemap chains when transforming
 - rollup@function@non-function-hook-async: throws when providing a value for an async function hook
 - rollup@function@non-function-hook-sync: throws when providing a value for a sync function hook
 - rollup@function@export-type-mismatch-b: export type must be auto, default, named or none
 - rollup@function@circular-default-exports: handles circular default exports
 - rollup@function@assign-namespace-to-var: allows a namespace to be assigned to a variable (chunk empty warning)
 - rollup@function@can-import-self-treeshake: direct self import (chunk empty warning)
 - rollup@function@external-conflict: external paths from custom resolver remain external (#633)
 - rollup@function@shims-missing-exports: shims missing exports
 - rollup@function@nested-inlined-dynamic-import-2: deconflicts variables when nested dynamic imports are inlined
 - rollup@function@already-deshadowed-import: handle already module import names correctly if they are have already been deshadowed
 - rollup@function@can-import-self: a module importing its own bindings
 - rollup@function@conflicting-reexports@named-import-external: warns when a conflicting binding is imported via a named import from external namespaces
 - rollup@function@cycles-stack-overflow: does not stack overflow on crazy cyclical dependencies
 - rollup@function@cycles-default-anonymous-function-hoisted: Anonymous function declarations are hoisted
 - rollup@function@cycles-immediate: handles cycles where imports are immediately used
 - rollup@function@cycles-pathological-2: resolves even more pathological cyclical dependencies gracefully
 - rollup@function@cycles-defaults: cycles work with default exports
 - rollup@function@cycles-export-star: does not stack overflow on `export * from X` cycles
 - rollup@function@circular-missed-reexports: handles circular reexports
 - rollup@function@circular-missed-reexports-2: handles circular reexports
 - rollup@function@dynamic-import-relative-not-found: throws if a dynamic relative import is not found
 - rollup@function@error-after-transform-should-throw-correct-location: error after transform should throw with correct location of file
 - rollup@function@error-parse-unknown-extension: throws with an extended error message when failing to parse a file without .(m)js extension
 - rollup@function@error-parse-json: throws with an extended error message when failing to parse a file with ".json" extension
 - rollup@function@iife-code-splitting: throws when generating multiple chunks for an IIFE build
 - rollup@function@import-of-unexported-fails: marking an imported, but unexported, identifier should throw
 - rollup@function@inline-imports-with-multiple-array: Having multiple inputs in an array is not supported when inlining dynamic imports
 - rollup@function@inline-imports-with-multiple-object: Having multiple inputs in an object is not supported when inlining dynamic imports
 - rollup@function@warning-low-resolution-location: handles when a low resolution sourcemap is used to report an error
 - rollup@function@warning-incorrect-sourcemap-location: does not fail if a warning has an incorrect location due to missing sourcemaps
 - rollup@function@paths-are-case-sensitive: insists on correct casing for imports
 - rollup@function@recursive-reexports: handles recursive namespace reexports
 - rollup@function@self-referencing-namespace: supports dynamic namespaces that reference themselves
 - rollup@function@no-relative-external: missing relative imports are an error, not a warning
 - rollup@function@warnings-to-string: provides a string conversion for warnings
 - rollup@function@warn-on-empty-bundle: warns if empty bundle is generated  (#444)
 - rollup@function@warn-on-namespace-conflict: warns on duplicate export * from
 - rollup@function@warn-on-unused-missing-imports: warns on missing (but unused) imports
 - rollup@function@warn-misplaced-annotations: warns for misplaced annotations
 - rollup@function@namespace-missing-export: replaces missing namespace members with undefined and warns about them
 - rollup@function@throws-not-found-module: throws error if module is not found
 - rollup@function@transform-without-code-warn-ast: warns when returning a map but no code from a transform hook
 - rollup@function@transform-without-code-warn-map: warns when returning a map but no code from a transform hook
 - rollup@function@unknown-treeshake-value: throws for unknown string values for the treeshake option
 - rollup@function@warns-for-invalid-options: warns for invalid options
 - rollup@function@module-side-effects@invalid-option: warns for invalid options
 - rollup@function@invalid-addon-hook: throws when providing a non-string value for an addon hook
 - rollup@function@invalid-default-export-mode: throw for invalid default export mode
 - rollup@function@invalid-ignore-list-function: throw descriptive error if sourcemapIgnoreList-function does not return a boolean
 - rollup@function@invalid-transform-source-function: throw descriptive error if sourcemapPathTransform-function does not return a string (#3484)
 - rollup@function@invalid-pattern-replacement: throws for invalid placeholders in patterns
 - rollup@function@invalid-pattern: throws for invalid patterns
 - rollup@function@invalid-top-level-await: throws for invalid top-level-await format
 - rollup@function@load-returns-string-or-null: throws error if load returns something wacky    
 - rollup@function@vars-with-init-in-dead-branch: handles vars with init in dead branch (#1198)
 - rollup@function@update-expression-of-import-fails: disallows updates to imported bindings
 - rollup@function@reassign-import-not-at-top-level-fails: disallows assignments to imported bindings not at the top level
 - rollup@function@reassign-import-fails: disallows assignments to imported bindings
 - rollup@function@unused-import: warns on unused imports ([#595])
 - rollup@function@module-level-directive: module level directives should produce warnings    
 - rollup@function@import-not-at-top-level-fails: disallows non-top-level imports
 - rollup@function@export-not-at-top-level-fails: disallows non-top-level exports
 - rollup@function@hashing@maximum-hash-size: throws when the maximum hash size is exceeded
 - rollup@function@hashing@minimum-hash-size: throws when the maximum hash size is exceeded
 - rollup@function@hashing@length-at-non-hash: throws when configuring a length for placeholder other than "hash"
 - rollup@function@emit-file@invalid-file-type: throws for invalid file types
 - rollup@function@emit-file@invalid-asset-name3: throws for invalid asset names with absolute path on Windows OS
 - rollup@function@emit-file@invalid-asset-name: throws for invalid asset names
 - rollup@function@emit-file@emit-same-file: warns if multiple files with the same name are emitted
 - rollup@function@emit-file@emit-from-output-options: throws when trying to emit files from the outputOptions hook
 - rollup@function@duplicate-import-specifier-fails: disallows duplicate import specifiers
 - rollup@function@duplicate-import-fails: disallows duplicate imports
 - rollup@function@double-named-export: throws on duplicate named exports
 - rollup@function@double-named-reexport: throws on duplicate named exports
 - rollup@function@double-default-export: throws on double default exports
 - rollup@function@deprecations@externalImportAssertions: marks the "output.externalImportAssertions" option as deprecated
 - rollup@function@cannot-call-external-namespace: warns if code calls an external namespace
 - rollup@function@cannot-call-internal-namespace: warns if code calls an internal namespace
 - rollup@function@circular-reexport: throws proper error for circular reexports
 - rollup@function@conflicting-reexports@namespace-import: warns when a conflicting binding is imported via a namespace import 
 - rollup@function@cannot-resolve-sourcemap-warning: handles when a sourcemap cannot be resolved in a warning
 - rollup@function@adds-json-hint-for-missing-export-if-is-json-file: should provide json hint when importing a no export json file
 - rollup@function@add-watch-file-generate: throws when adding watch files during generate
 - rollup@function@input-name-validation2: throws for relative paths as input names
 - rollup@function@input-name-validation3: throws for relative paths as input names
 - rollup@function@input-name-validation: throws for absolute paths as input names
 - rollup@function@emit-file@asset-source-invalid: throws when setting an empty asset source
 - rollup@function@emit-file@asset-source-missing3: throws when accessing the file name before the asset source is set
 - rollup@function@emit-file@asset-source-missing4: throws when accessing the file name before the asset source is set
 - rollup@function@emit-file@asset-source-missing2: throws when not setting the asset source
 - rollup@function@emit-file@asset-source-missing5: throws when not setting the asset source and accessing the asset URL
 - rollup@function@emit-file@asset-source-missing: throws when not setting the asset source
 - rollup@function@emit-file@invalid-reference-id: throws for invalid reference ids
