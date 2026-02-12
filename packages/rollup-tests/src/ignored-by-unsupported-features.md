# Tests failed by unsupported features

## Plugin related

### The `NormalziedOptions` at hooks is not compatible with rollup
 - rollup@function@options-hook: allows to read and modify options in the options hook
 - rollup@function@output-options-hook: allows to read and modify options in the options hook

### The `load` hook return `ast` is not supported
 - rollup@function@uses-supplied-ast: uses supplied AST
 - rollup@form@custom-ast: supports returning a custom AST from a plugin

### The `resolveId` hook `resolvedBy` is not supported
 - rollup@function@validate-resolved-by-logic: validate resolvedBy logic

### The `shouldTransformCachedModule` hook is not supported
 - rollup@function@plugin-error-should-transform: errors in shouldTransformCachedModule abort the build

### The `resolveDynamicImport` hook `specifier: AstNode` not supported
 - rollup@form@dynamic-import-unresolvable: Returns the raw AST nodes for unresolvable dynamic imports@generates es
 - rollup@function@dynamic-import-expression: Dynamic import expression replacement

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
 - rollup@function@implicit-dependencies@dependant-dynamic-import-no-effects: throws when a module that is loaded before an emitted chunk is fully tree-shaken
 - rollup@function@implicit-dependencies@dependant-dynamic-import-not-included: throws when a module that is loaded before an emitted chunk is only linked to the module graph via a tree-shaken dynamic import
 - rollup@function@implicit-dependencies@dependant-not-part-of-graph: throws when a module that is loaded before an emitted chunk is not part of the module graph
 - rollup@function@implicit-dependencies@external-dependant: throws when a module that is loaded before an emitted chunk does not exist
 - rollup@function@implicit-dependencies@missing-dependant: throws when a module that is loaded before an emitted chunk is external
 - rollup@function@emit-file@set-asset-source-chunk: throws when trying to set the asset source of a chunk
 - rollup@function@emit-file@modules-loaded: Throws when adding a chunk after the modules have finished loading
 - rollup@function@emit-file@invalid-chunk-id: throws for invalid chunk ids
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

### `originalFileName` / `originalFileNames` is not supported properly
- rollup@function@deprecated@emit-file@original-file-name: forwards the original file name to other hooks
- rollup@function@emit-file@original-file-name: forwards the original file name to other hooks
- rollup@function@emit-file@original-file-names: forwards the original file name to other hooks

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
 - rollup@form@system-module-reserved: does not output reserved system format identifiers@generates es

### The `input.perf` and `bundle.getTimings()` is not supported
 - rollup@function@adds-timings-to-bundle-when-codesplitting: Adds timing information to bundle when bundling with perf=true
 - rollup@function@adds-timings-to-bundle: Adds timing information to bundle when bundling with perf=true

### The `input.moduleContext` is not supported
 - rollup@form@custom-module-context-function: allows custom module-specific context with a function option
 - rollup@form@custom-module-context: allows custom module-specific context@generates es

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
 - rollup@form@generated-code@arrow-functions-false: does not use arrow functions@generates es
 - rollup@form@generated-code@arrow-functions-true: uses arrow functions@generates es
 - rollup@form@generated-code@const-bindings-false: does not use block bindings@generates es
 - rollup@form@generated-code@const-bindings-true: uses block bindings@generates es
 - rollup@form@generated-code@object-shorthand-false: does not use object shorthand syntax
 - rollup@form@generated-code@object-shorthand-true: uses object shorthand syntax
 - rollup@form@generated-code@reserved-names-as-props-false: escapes reserved names used as props@generates es
 - rollup@form@generated-code@reserved-names-as-props-true: escapes reserved names used as props@generates es
 - rollup@function@unknown-generated-code-value: throws for unknown string values for the generatedCode option

### The `output.generatedCode.preset` is not supported
 - rollup@form@generated-code-presets@es2015: handles generatedCode preset "es2015"
 - rollup@form@generated-code-presets@es5: handles generatedCode preset "es5"
 - rollup@form@generated-code-presets@preset-with-override: handles generatedCode preset "es2015"
 - rollup@function@unknown-generated-code-preset: throws for unknown presets for the generatedCode option

### The `output.generatedCode.symbols` is not supported properly
 - rollup@function@name-conflict-symbol: avoids name conflicts with local variables named Symbol
 - rollup@function@namespace-tostring@dynamic-import-default-mode: adds Symbol.toStringTag property to dynamic imports of entry chunks with default export mode
 - rollup@function@namespace-tostring@dynamic-import: adds Symbol.toStringTag property to dynamic imports
 - rollup@function@namespace-tostring@external-namespaces: adds Symbol.toStringTag property to external namespaces
 - rollup@function@namespace-tostring@property-descriptor: namespace export should have @@toStringTag with correct property descriptors #4336

### The `output.preserveModules` is not compatible yet
 - rollup@function@preserve-modules-default-mode-namespace: import namespace from chunks with default export mode when preserving modules,
 - rollup@function@circular-preserve-modules: correctly handles circular dependencies when preserving modules
 - rollup@function@missing-export-preserve-modules: supports shimming missing exports when preserving modules
 - rollup@function@preserve-modules-circular-order: preserves execution order for circular dependencies when preserving modules
 - rollup@function@preserve-modules@invalid-default-export-mode: throws when using default export mode with named exports
 - rollup@function@preserve-modules@invalid-no-preserve-entry-signatures: throws when setting preserveEntrySignatures to false
 - rollup@function@preserve-modules@invalid-none-export-mode: throws when using none export mode with named exports
 - rollup@function@preserve-modules@manual-chunks: Assigning manual chunks fails when preserving modules
 - rollup@function@preserve-modules@mixed-exports: warns for mixed exports in all chunks when preserving modules
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
 - rollup@function@manual-chunks-order: sorts manual chunks by entry index

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

## Features

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
 - rollup@form@namespace-optimization-in-operator-synthetic: disables optimization for synthetic named exports when using the in operator

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
 - rollup@form@import-attributes@keep-attribute-declarations-for-external-dynamic-imports: Keep the attribute declarations for external dynamic imports
 - rollup@form@import-attributes@keep-dynamic-attributes-assert: keep import attributes for dynamic imports with "assert" key@generates es
 - rollup@form@import-attributes@keep-dynamic-attributes-default: keep import attributes for dynamic imports@generates es
 - rollup@form@import-attributes@keep-dynamic-attributes-with: keep import attributes for dynamic imports with "with" key@generates es
 - rollup@form@import-attributes@keeps-static-attributes-key-assert: keeps any import attributes on input using import attributes with "with" key@generates es
 - rollup@form@import-attributes@keeps-static-attributes-key-default: keeps any import attributes on input using import attributes with "with" key@generates es
 - rollup@form@import-attributes@keeps-static-attributes-key-with: keeps any import attributes on input using import attributes with "with" key@generates es
 - rollup@form@resolve-file-url-import-meta-attributes: adds attributes to file resolveFileUrl and resolveImportMeta hooks@generates es
 - rollup@function@deprecated@load-attributes: does not allow returning attributes from the "load" hook
 - rollup@function@deprecated@transform-attributes: does not allow returning attributes from the "transform" hook
 - rollup@function@extend-more-hooks-to-include-import-attributes: extend load, transform and renderDynamicImport to include import attributes

### watch behavior is not compatible yet
 - rollup@hooks@allows to enforce plugin hook order in watch mode

### escaping external id is not supported
 - rollup@form@quote-id: handles escaping for external ids@generates es

### remove `use strict` from function body
 - rollup@function@function-use-strict-directive-removed: should delete use strict from function body

### The namespace object is not compatible with rollup
 - rollup@function@namespaces-have-null-prototype: creates namespaces with null prototypes
 - rollup@function@namespaces-are-frozen: namespaces should be non-extensible and its properties immutatable and non-configurable
 - rollup@function@namespace-override: does not warn when overriding namespace reexports with explicit ones
 - rollup@function@escape-arguments: does not use "arguments" as a placeholder variable for a default export
 - rollup@function@dynamic-import-only-default: correctly imports dynamic namespaces with only a default export from entry- and non-entry-point chunks
 - rollup@function@dynamic-import-default-mode-facade: handles dynamic imports from facades using default export mode
 - rollup@function@chunking-duplicate-reexport: handles duplicate reexports when using dynamic imports
 - rollup@function@namespace-tostring@interop-property-descriptor: generated interop namespaces should have correct Symbol.toStringTag
 - rollup@function@external-dynamic-import-live-binding-compact: supports external dynamic imports with live bindings in compact mode
 - rollup@function@external-dynamic-import-live-binding: supports external dynamic imports with live bindings
 - rollup@function@no-external-live-bindings: Allows omitting the code that handles external live bindings
 - rollup@function@no-external-live-bindings-compact: Allows omitting the code that handles external live bindings

### `hasOwnProperty` export is not handled properly
 - rollup@function@re-export-own: avoid using export.hasOwnProperty (hasOwnProperty behavior differs)

### `__proto__` export is not properly handled
 - rollup@form@cjs-transpiler-re-exports-1: Disable reexporting the __proto__ from the external module if both output.externalLiveBindings and output.reExportProtoFromExternal are false@generates cjs
 - rollup@form@cjs-transpiler-re-exports: Compatibility with CJS Transpiler Re-exports if output.externalLiveBindings is false@generates cjs

### source map combine logic does not support coarse sourcemap well enough
- rollup@sourcemaps@combined-sourcemap-3: get correct combined sourcemap of bundled code@generates es

### `strictDeprecations` option is not supported
 - rollup@function@deprecations@externalImportAssertions: marks the "output.externalImportAssertions" option as deprecated
 - rollup@function@deprecations@asset-filename-name: marks the "name" property of emitted assets as deprecated in assetFileNames
 - rollup@function@deprecations@asset-filename-originalfilename: marks the "name" property of emitted assets as deprecated in assetFileNames
 - rollup@function@deprecations@asset-name-in-bundle: marks the "name" property of emitted assets as deprecated in generateBundle
 - rollup@function@deprecations@asset-originalfilename-in-bundle: marks the "originalFileName" property of emitted assets as deprecated in generateBundle when emitted during generate phase
 - rollup@function@deprecations@asset-render-chunk-originalfilename-in-bundle: marks the "originalFileName" property of emitted assets as deprecated in generateBundle when emitted during generate phase
 - rollup@function@deprecations@asset-render-chunk-name-in-bundle: marks the "name" property of emitted assets as deprecated in generateBundle when emitted during generate phase

### The error/warning information is not compatible with rollup
 - rollup@function@banner-and-footer: adds a banner/footer (expects `ADDON_ERROR` but got `PLUGIN_ERROR`)
 - rollup@function@conflicting-reexports@named-import: throws when a conflicting binding is imported via a named import (expects `AMBIGUOUS_EXTERNAL_NAMESPACES` but got `MISSING_EXPORT`)
 - rollup@function@logging@handle-logs-in-plugins: allows plugins to read and filter logs
 - rollup@hooks@supports renderError hook
 - rollup@function@ast-validations@redeclare-catch-scope-parameter-var-outside-conflict: throws when redeclaring a parameter of a catch scope as a var that conflicts with an outside binding (unknown)
 - rollup@function@import-not-at-top-level-fails: disallows non-top-level imports (`cause` property is missing)
 - rollup@function@export-not-at-top-level-fails: disallows non-top-level exports (`cause` property is missing)

### The error/warning not implement
 - rollup@hooks@Throws when using the "sourcemapFile" option for multiple chunks (`INVALID_OPTION` error)
 - rollup@function@transform-without-sourcemap-render-chunk: preserves sourcemap chains when transforming (`SOURCEMAP_BROKEN` warning)
 - rollup@function@non-function-hook-async: throws when providing a value for an async function hook (expected `INVALID_PLUGIN_HOOK` error, but got `PLUGIN_ERROR`)
 - rollup@function@non-function-hook-sync: throws when providing a value for a sync function hook (`INVALID_PLUGIN_HOOK` error)
 - rollup@function@export-type-mismatch-b: export type must be auto, default, named or none (expected `INVALID_EXPORT_OPTION` error, but got `InvalidArg`)
 - rollup@function@assign-namespace-to-var: allows a namespace to be assigned to a variable (`EMPTY_BUNDLE` warning)
 - rollup@function@can-import-self-treeshake: direct self import (`EMPTY_BUNDLE` warning)
 - rollup@function@external-conflict: external paths from custom resolver remain external (#633) (`INVALID_EXTERNAL_ID` error)
 - rollup@function@shims-missing-exports: shims missing exports (`SHIMMED_EXPORT` warning)
 - rollup@function@conflicting-reexports@named-import-external: warns when a conflicting binding is imported via a named import from external namespaces (`AMBIGUOUS_EXTERNAL_NAMESPACES`, `UNUSED_EXTERNAL_IMPORT` warning)
 - rollup@function@cycles-pathological-2: resolves even more pathological cyclical dependencies gracefully
 - rollup@function@circular-missed-reexports: handles circular reexports (`MISSING_EXPORT` should be warning instead of error)
 - rollup@function@iife-code-splitting: throws when generating multiple chunks for an IIFE build (`INVALID_OPTION` error)
 - rollup@function@inline-imports-with-multiple-array: Having multiple inputs in an array is not supported when inlining dynamic imports (expected `INVALID_OPTION`, but got `GenericFailure`)
 - rollup@function@inline-imports-with-multiple-object: Having multiple inputs in an object is not supported when inlining dynamic imports (expected `INVALID_OPTION`, but got `GenericFailure`)
 - rollup@function@preserve-modules@inline-dynamic-imports: Inlining dynamic imports is not supported when preserving modules (expected `INVALID_OPTION`, but got `GenericFailure`)
 - rollup@function@inline-imports-with-manual: Manual chunks are not supported when inlining dynamic imports (`INVALID_OPTION` error)
 - rollup@function@warning-low-resolution-location: handles when a low resolution sourcemap is used to report an error (`THIS_IS_UNDEFINED` warning)
 - rollup@function@warning-incorrect-sourcemap-location: does not fail if a warning has an incorrect location due to missing sourcemaps (expected `MISSING_EXPORT` warning, but got `IMPORT_IS_UNDEFINED`)
 - rollup@function@paths-are-case-sensitive: insists on correct casing for imports
 - rollup@function@warnings-to-string: provides a string conversion for warnings (`EMPTY_BUNDLE` warning)
 - rollup@function@warn-on-empty-bundle: warns if empty bundle is generated  (#444) (`EMPTY_BUNDLE` warning)
 - rollup@function@warn-on-namespace-conflict: warns on duplicate export * from (`NAMESPACE_CONFLICT` warning)
 - rollup@function@warn-on-unused-missing-imports: warns on missing (but unused) imports (`MISSING_EXPORT` should be warning instead of an error)
 - rollup@function@warn-misplaced-annotations: warns for misplaced annotations (`INVALID_ANNOTATION` warning)
 - rollup@function@namespace-missing-export: replaces missing namespace members with undefined and warns about them (expected `MISSING_EXPORT` warning, but got `IMPORT_IS_UNDEFINED`)
 - rollup@function@transform-without-code-warn-ast: warns when returning a map but no code from a transform hook (`NO_TRANSFORM_MAP_OR_AST_WITHOUT_CODE` warning)
 - rollup@function@transform-without-code-warn-map: warns when returning a map but no code from a transform hook (`NO_TRANSFORM_MAP_OR_AST_WITHOUT_CODE` warning)
 - rollup@function@unknown-treeshake-value: throws for unknown string values for the treeshake option (`INVALID_OPTION` error)
 - rollup@function@warns-for-invalid-options: warns for invalid options (`UNKNOWN_OPTION` warning)
 - rollup@function@module-side-effects@invalid-option: warns for invalid options (`INVALID_OPTION` error expected, but got `InvalidArg`)
 - rollup@function@invalid-addon-hook: throws when providing a non-string value for an addon hook (`ADDON_ERROR` error expected, but got `unreachable: Invalid hook type`)
 - rollup@function@invalid-ignore-list-function: throw descriptive error if sourcemapIgnoreList-function does not return a boolean (`VALIDATION_ERROR` error expected, but got `InvalidArg`)
 - rollup@function@invalid-transform-source-function: throw descriptive error if sourcemapPathTransform-function does not return a string (#3484) (`VALIDATION_ERROR` error expected, but got `GenericFailure`)
 - rollup@function@invalid-pattern-replacement: throws for invalid placeholders in patterns (`VALIDATION_ERROR` error)
 - rollup@function@invalid-pattern: throws for invalid patterns (`VALIDATION_ERROR` error expected, but got `INVALID_OPTION`)
 - rollup@function@invalid-top-level-await: throws for invalid top-level-await format (`INVALID_TLA_FORMAT` error expected, but got `UNSUPPORTED_FEATURE`)
 - rollup@function@load-returns-string-or-null: throws error if load returns something wacky (`BAD_LOADER` error expected, but got `InvalidArg`)
 - rollup@function@vars-with-init-in-dead-branch: handles vars with init in dead branch (#1198) (`EMPTY_BUNDLE` warning)
 - rollup@function@unused-import: warns on unused imports ([#595]) (`UNUSED_EXTERNAL_IMPORT` warning)
 - rollup@function@unused-import-2: warns on unused imports ([#595]) (`UNUSED_EXTERNAL_IMPORT` warning)
 - rollup@function@module-level-directive: module level directives should produce warnings (`MODULE_LEVEL_DIRECTIVE` warning)
 - rollup@function@hashing@maximum-hash-size: throws when the maximum hash size is exceeded (`VALIDATION_ERROR` error)
 - rollup@function@hashing@minimum-hash-size: throws when the maximum hash size is exceeded (`VALIDATION_ERROR` error)
 - rollup@function@hashing@length-at-non-hash: throws when configuring a length for placeholder other than "hash" (`VALIDATION_ERROR` error)
 - rollup@function@emit-file@invalid-file-type: throws for invalid file types (`pluginCode":"VALIDATION_ERROR"` expected, but got `pluginCode:"InvalidArg"`)
 - rollup@function@emit-file@invalid-asset-name3: throws for invalid asset names with absolute path on Windows OS (`PLUGIN_ERROR`>`VALIDATION_ERROR` error)
 - rollup@function@emit-file@invalid-asset-name: throws for invalid asset names (`PLUGIN_ERROR`>`VALIDATION_ERROR` error)
 - rollup@function@emit-file@emit-same-file: warns if multiple files with the same name are emitted (`FILE_NAME_CONFLICT` error)
 - rollup@function@emit-file@emit-from-output-options: throws when trying to emit files from the outputOptions hook (`CANNOT_EMIT_FROM_OPTIONS_HOOK` error)
 - rollup@function@conflicting-reexports@namespace-import: warns when a conflicting binding is imported via a namespace import (`MISSING_EXPORT` warning)
 - rollup@function@cannot-resolve-sourcemap-warning: handles when a sourcemap cannot be resolved in a warning (`SOURCEMAP_ERROR` warning)
 - rollup@function@adds-json-hint-for-missing-export-if-is-json-file: should provide json hint when importing a no export json file (`pluginCode":"VALIDATION_ERROR"` expected, but got `pluginCode:"InvalidArg"`)
 - rollup@function@emit-file@asset-source-invalid: throws when setting an empty asset source (`pluginCode":"VALIDATION_ERROR"` expected, but got `pluginCode:"InvalidArg"`)
 - rollup@function@emit-file@asset-source-missing3: throws when accessing the file name before the asset source is set (`ASSET_SOURCE_MISSING` error is expected, but `PLUGIN_ERROR` is thrown)
 - rollup@function@emit-file@asset-source-missing4: throws when accessing the file name before the asset source is set (`ASSET_SOURCE_MISSING` error is expected, but `PLUGIN_ERROR` is thrown)
 - rollup@function@emit-file@asset-source-missing2: throws when not setting the asset source (`ASSET_SOURCE_MISSING` error is expected, but `PLUGIN_ERROR` is thrown)
 - rollup@function@emit-file@asset-source-missing5: throws when not setting the asset source and accessing the asset URL (`ASSET_SOURCE_MISSING` error is expected, but `PLUGIN_ERROR` is thrown)
 - rollup@function@emit-file@asset-source-missing: throws when not setting the asset source (`ASSET_SOURCE_MISSING` error is expected, but `PLUGIN_ERROR` is thrown)
 - rollup@function@emit-file@invalid-reference-id: throws for invalid reference ids (missing error)
 - rollup@form@cycles-dependency-with-TLA-await-import: throw a warn when a cycle is detected which includes a top-level await import (`CIRCULAR_DEPENDENCY` warning)
 - rollup@function@optional-chaining-namespace: handles optional chaining with namespace (`UNUSED_EXTERNAL_IMPORT` warning)
 - rollup@function@ast-validations@redeclare-import-var: throws when redeclaring an import with a var (https://github.com/oxc-project/oxc/issues/15961)
 - rollup@function@warn-on-top-level-this: warns on top-level this (#770) (`THIS_IS_UNDEFINED` warning)
 - rollup@sourcemaps@warning-with-coarse-sourcemap: get correct mapping location with coarse sourcemap@generates es (`THIS_IS_UNDEFINED` warning)
