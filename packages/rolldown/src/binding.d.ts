type MaybePromise<T> = T | Promise<T>
type Nullable<T> = T | null | undefined
type VoidNullable<T = void> = T | null | undefined | void
export type BindingStringOrRegex = string | RegExp

export interface CodegenOptions {
  /**
   * Remove whitespace.
   *
   * @default true
   */
  removeWhitespace?: boolean
}

export interface CompressOptions {
  /**
   * Set desired EcmaScript standard version for output.
   *
   * Set `esnext` to enable all target highering.
   *
   * e.g.
   *
   * * catch optional binding when >= es2019
   * * `??` operator >= es2020
   *
   * @default 'esnext'
   */
  target?: 'esnext' | 'es2015' | 'es2016' | 'es2017' | 'es2018' | 'es2019' | 'es2020' | 'es2021' | 'es2022' | 'es2023' | 'es2024'
  /**
   * Pass true to discard calls to `console.*`.
   *
   * @default false
   */
  dropConsole?: boolean
  /**
   * Remove `debugger;` statements.
   *
   * @default true
   */
  dropDebugger?: boolean
  /**
   * Drop unreferenced functions and variables.
   *
   * Simple direct variable assignments do not count as references unless set to "keep_assign".
   */
  unused?: true | false | 'keep_assign'
  /** Keep function / class names. */
  keepNames?: CompressOptionsKeepNames
}

export interface CompressOptionsKeepNames {
  /**
   * Keep function names so that `Function.prototype.name` is preserved.
   *
   * This does not guarantee that the `undefined` name is preserved.
   *
   * @default false
   */
  function: boolean
  /**
   * Keep class names so that `Class.prototype.name` is preserved.
   *
   * This does not guarantee that the `undefined` name is preserved.
   *
   * @default false
   */
  class: boolean
}

export interface MangleOptions {
  /**
   * Pass `true` to mangle names declared in the top level scope.
   *
   * @default false
   */
  toplevel?: boolean
  /**
   * Preserve `name` property for functions and classes.
   *
   * @default false
   */
  keepNames?: boolean | MangleOptionsKeepNames
  /** Debug mangled names. */
  debug?: boolean
}

export interface MangleOptionsKeepNames {
  /**
   * Preserve `name` property for functions.
   *
   * @default false
   */
  function: boolean
  /**
   * Preserve `name` property for classes.
   *
   * @default false
   */
  class: boolean
}

/** Minify synchronously. */
export declare function minify(filename: string, sourceText: string, options?: MinifyOptions | undefined | null): MinifyResult

export interface MinifyOptions {
  /** Use when minifying an ES6 module. */
  module?: boolean
  compress?: boolean | CompressOptions
  mangle?: boolean | MangleOptions
  codegen?: boolean | CodegenOptions
  sourcemap?: boolean
}

export interface MinifyResult {
  code: string
  map?: SourceMap
  errors: Array<OxcError>
}
export interface Comment {
  type: 'Line' | 'Block'
  value: string
  start: number
  end: number
}

export interface ErrorLabel {
  message?: string
  start: number
  end: number
}

export interface OxcError {
  severity: Severity
  message: string
  labels: Array<ErrorLabel>
  helpMessage?: string
  codeframe?: string
}

export type Severity =  'Error'|
'Warning'|
'Advice';
export declare class ParseResult {
  get program(): import("@oxc-project/types").Program
  get module(): EcmaScriptModule
  get comments(): Array<Comment>
  get errors(): Array<OxcError>
}

export interface DynamicImport {
  start: number
  end: number
  moduleRequest: Span
}

export interface EcmaScriptModule {
  /**
   * Has ESM syntax.
   *
   * i.e. `import` and `export` statements, and `import.meta`.
   *
   * Dynamic imports `import('foo')` are ignored since they can be used in non-ESM files.
   */
  hasModuleSyntax: boolean
  /** Import statements. */
  staticImports: Array<StaticImport>
  /** Export statements. */
  staticExports: Array<StaticExport>
  /** Dynamic import expressions. */
  dynamicImports: Array<DynamicImport>
  /** Span positions` of `import.meta` */
  importMetas: Array<Span>
}

export interface ExportExportName {
  kind: ExportExportNameKind
  name?: string
  start?: number
  end?: number
}

export type ExportExportNameKind = /** `export { name } */
'Name'|
/** `export default expression` */
'Default'|
/** `export * from "mod" */
'None';

export interface ExportImportName {
  kind: ExportImportNameKind
  name?: string
  start?: number
  end?: number
}

export type ExportImportNameKind = /** `export { name } */
'Name'|
/** `export * as ns from "mod"` */
'All'|
/** `export * from "mod"` */
'AllButDefault'|
/** Does not have a specifier. */
'None';

export interface ExportLocalName {
  kind: ExportLocalNameKind
  name?: string
  start?: number
  end?: number
}

export type ExportLocalNameKind = /** `export { name } */
'Name'|
/** `export default expression` */
'Default'|
/**
 * If the exported value is not locally accessible from within the module.
 * `export default function () {}`
 */
'None';

export interface ImportName {
  kind: ImportNameKind
  name?: string
  start?: number
  end?: number
}

export type ImportNameKind = /** `import { x } from "mod"` */
'Name'|
/** `import * as ns from "mod"` */
'NamespaceObject'|
/** `import defaultExport from "mod"` */
'Default';

/**
 * Parse asynchronously.
 *
 * Note: This function can be slower than `parseSync` due to the overhead of spawning a thread.
 */
export declare function parseAsync(filename: string, sourceText: string, options?: ParserOptions | undefined | null): Promise<ParseResult>

export interface ParserOptions {
  /** Treat the source text as `js`, `jsx`, `ts`, `tsx` or `dts`. */
  lang?: 'js' | 'jsx' | 'ts' | 'tsx' | 'dts'
  /** Treat the source text as `script` or `module` code. */
  sourceType?: 'script' | 'module' | 'unambiguous' | undefined
  /**
   * Return an AST which includes TypeScript-related properties, or excludes them.
   *
   * `'js'` is default for JS / JSX files.
   * `'ts'` is default for TS / TSX files.
   * The type of the file is determined from `lang` option, or extension of provided `filename`.
   */
  astType?: 'js' | 'ts'
  /**
   * Controls whether the `range` property is included on AST nodes.
   * The `range` property is a `[number, number]` which indicates the start/end offsets
   * of the node in the file contents.
   *
   * @default false
   */
  range?: boolean
  /**
   * Emit `ParenthesizedExpression` and `TSParenthesizedType` in AST.
   *
   * If this option is true, parenthesized expressions are represented by
   * (non-standard) `ParenthesizedExpression` and `TSParenthesizedType` nodes that
   * have a single `expression` property containing the expression inside parentheses.
   *
   * @default true
   */
  preserveParens?: boolean
  /**
   * Produce semantic errors with an additional AST pass.
   * Semantic errors depend on symbols and scopes, where the parser does not construct.
   * This adds a small performance overhead.
   *
   * @default false
   */
  showSemanticErrors?: boolean
}

/** Parse synchronously. */
export declare function parseSync(filename: string, sourceText: string, options?: ParserOptions | undefined | null): ParseResult

/** Returns `true` if raw transfer is supported on this platform. */
export declare function rawTransferSupported(): boolean

export interface Span {
  start: number
  end: number
}

export interface StaticExport {
  start: number
  end: number
  entries: Array<StaticExportEntry>
}

export interface StaticExportEntry {
  start: number
  end: number
  moduleRequest?: ValueSpan
  /** The name under which the desired binding is exported by the module`. */
  importName: ExportImportName
  /** The name used to export this binding by this module. */
  exportName: ExportExportName
  /** The name that is used to locally access the exported value from within the importing module. */
  localName: ExportLocalName
  /**
   * Whether the export is a TypeScript `export type`.
   *
   * Examples:
   *
   * ```ts
   * export type * from 'mod';
   * export type * as ns from 'mod';
   * export type { foo };
   * export { type foo }:
   * export type { foo } from 'mod';
   * ```
   */
  isType: boolean
}

export interface StaticImport {
  /** Start of import statement. */
  start: number
  /** End of import statement. */
  end: number
  /**
   * Import source.
   *
   * ```js
   * import { foo } from "mod";
   * //                   ^^^
   * ```
   */
  moduleRequest: ValueSpan
  /**
   * Import specifiers.
   *
   * Empty for `import "mod"`.
   */
  entries: Array<StaticImportEntry>
}

export interface StaticImportEntry {
  /**
   * The name under which the desired binding is exported by the module.
   *
   * ```js
   * import { foo } from "mod";
   * //       ^^^
   * import { foo as bar } from "mod";
   * //       ^^^
   * ```
   */
  importName: ImportName
  /**
   * The name that is used to locally access the imported value from within the importing module.
   * ```js
   * import { foo } from "mod";
   * //       ^^^
   * import { foo as bar } from "mod";
   * //              ^^^
   * ```
   */
  localName: ValueSpan
  /**
   * Whether this binding is for a TypeScript type-only import.
   *
   * `true` for the following imports:
   * ```ts
   * import type { foo } from "mod";
   * import { type foo } from "mod";
   * ```
   */
  isType: boolean
}

export interface ValueSpan {
  value: string
  start: number
  end: number
}
export declare class ResolverFactory {
  constructor(options?: NapiResolveOptions | undefined | null)
  static default(): ResolverFactory
  /** Clone the resolver using the same underlying cache. */
  cloneWithOptions(options: NapiResolveOptions): ResolverFactory
  /** Clear the underlying cache. */
  clearCache(): void
  /** Synchronously resolve `specifier` at an absolute path to a `directory`. */
  sync(directory: string, request: string): ResolveResult
  /** Asynchronously resolve `specifier` at an absolute path to a `directory`. */
  async(directory: string, request: string): Promise<ResolveResult>
}

/** Node.js builtin module when `Options::builtin_modules` is enabled. */
export interface Builtin {
  /**
   * Resolved module.
   *
   * Always prefixed with "node:" in compliance with the ESM specification.
   */
  resolved: string
  /**
   * Whether the request was prefixed with `node:` or not.
   * `fs` -> `false`.
   * `node:fs` returns `true`.
   */
  isRuntimeModule: boolean
}

export declare enum EnforceExtension {
  Auto = 0,
  Enabled = 1,
  Disabled = 2
}

export type ModuleType =  'module'|
'commonjs'|
'json'|
'wasm'|
'addon';

/**
 * Module Resolution Options
 *
 * Options are directly ported from [enhanced-resolve](https://github.com/webpack/enhanced-resolve#resolver-options).
 *
 * See [webpack resolve](https://webpack.js.org/configuration/resolve/) for information and examples
 */
export interface NapiResolveOptions {
  /**
   * Path to TypeScript configuration file.
   *
   * Default `None`
   */
  tsconfig?: TsconfigOptions
  /**
   * Alias for [ResolveOptions::alias] and [ResolveOptions::fallback].
   *
   * For the second value of the tuple, `None -> AliasValue::Ignore`, Some(String) ->
   * AliasValue::Path(String)`
   * Create aliases to import or require certain modules more easily.
   * A trailing $ can also be added to the given object's keys to signify an exact match.
   * Default `{}`
   */
  alias?: Record<string, Array<string | undefined | null>>
  /**
   * A list of alias fields in description files.
   * Specify a field, such as `browser`, to be parsed according to [this specification](https://github.com/defunctzombie/package-browser-field-spec).
   * Can be a path to json object such as `["path", "to", "exports"]`.
   *
   * Default `[]`
   */
  aliasFields?: (string | string[])[]
  /**
   * Condition names for exports field which defines entry points of a package.
   * The key order in the exports field is significant. During condition matching, earlier entries have higher priority and take precedence over later entries.
   *
   * Default `[]`
   */
  conditionNames?: Array<string>
  /**
   * If true, it will not allow extension-less files.
   * So by default `require('./foo')` works if `./foo` has a `.js` extension,
   * but with this enabled only `require('./foo.js')` will work.
   *
   * Default to `true` when [ResolveOptions::extensions] contains an empty string.
   * Use `Some(false)` to disable the behavior.
   * See <https://github.com/webpack/enhanced-resolve/pull/285>
   *
   * Default None, which is the same as `Some(false)` when the above empty rule is not applied.
   */
  enforceExtension?: EnforceExtension
  /**
   * A list of exports fields in description files.
   * Can be a path to json object such as `["path", "to", "exports"]`.
   *
   * Default `[["exports"]]`.
   */
  exportsFields?: (string | string[])[]
  /**
   * Fields from `package.json` which are used to provide the internal requests of a package
   * (requests starting with # are considered internal).
   *
   * Can be a path to a JSON object such as `["path", "to", "imports"]`.
   *
   * Default `[["imports"]]`.
   */
  importsFields?: (string | string[])[]
  /**
   * An object which maps extension to extension aliases.
   *
   * Default `{}`
   */
  extensionAlias?: Record<string, Array<string>>
  /**
   * Attempt to resolve these extensions in order.
   * If multiple files share the same name but have different extensions,
   * will resolve the one with the extension listed first in the array and skip the rest.
   *
   * Default `[".js", ".json", ".node"]`
   */
  extensions?: Array<string>
  /**
   * Redirect module requests when normal resolving fails.
   *
   * Default `{}`
   */
  fallback?: Record<string, Array<string | undefined | null>>
  /**
   * Request passed to resolve is already fully specified and extensions or main files are not resolved for it (they are still resolved for internal requests).
   *
   * See also webpack configuration [resolve.fullySpecified](https://webpack.js.org/configuration/module/#resolvefullyspecified)
   *
   * Default `false`
   */
  fullySpecified?: boolean
  /**
   * A list of main fields in description files
   *
   * Default `["main"]`.
   */
  mainFields?: string | string[]
  /**
   * The filename to be used while resolving directories.
   *
   * Default `["index"]`
   */
  mainFiles?: Array<string>
  /**
   * A list of directories to resolve modules from, can be absolute path or folder name.
   *
   * Default `["node_modules"]`
   */
  modules?: string | string[]
  /**
   * Resolve to a context instead of a file.
   *
   * Default `false`
   */
  resolveToContext?: boolean
  /**
   * Prefer to resolve module requests as relative requests instead of using modules from node_modules directories.
   *
   * Default `false`
   */
  preferRelative?: boolean
  /**
   * Prefer to resolve server-relative urls as absolute paths before falling back to resolve in ResolveOptions::roots.
   *
   * Default `false`
   */
  preferAbsolute?: boolean
  /**
   * A list of resolve restrictions to restrict the paths that a request can be resolved on.
   *
   * Default `[]`
   */
  restrictions?: Array<Restriction>
  /**
   * A list of directories where requests of server-relative URLs (starting with '/') are resolved.
   * On non-Windows systems these requests are resolved as an absolute path first.
   *
   * Default `[]`
   */
  roots?: Array<string>
  /**
   * Whether to resolve symlinks to their symlinked location.
   * When enabled, symlinked resources are resolved to their real path, not their symlinked location.
   * Note that this may cause module resolution to fail when using tools that symlink packages (like npm link).
   *
   * Default `true`
   */
  symlinks?: boolean
  /**
   * Whether to parse [module.builtinModules](https://nodejs.org/api/module.html#modulebuiltinmodules) or not.
   * For example, "zlib" will throw [crate::ResolveError::Builtin] when set to true.
   *
   * Default `false`
   */
  builtinModules?: boolean
  /**
   * Resolve [ResolveResult::moduleType].
   *
   * Default `false`
   */
  moduleType?: boolean
  /**
   * Allow `exports` field in `require('../directory')`.
   *
   * This is not part of the spec but some vite projects rely on this behavior.
   * See
   * * <https://github.com/vitejs/vite/pull/20252>
   * * <https://github.com/nodejs/node/issues/58827>
   *
   * Default: `false`
   */
  allowPackageExportsInDirectoryResolve?: boolean
}

export interface ResolveResult {
  path?: string
  error?: string
  builtin?: Builtin
  /**
   * Module type for this path.
   *
   * Enable with `ResolveOptions#moduleType`.
   *
   * The module type is computed `ESM_FILE_FORMAT` from the [ESM resolution algorithm specification](https://nodejs.org/docs/latest/api/esm.html#resolution-algorithm-specification).
   *
   *  The algorithm uses the file extension or finds the closest `package.json` with the `type` field.
   */
  moduleType?: ModuleType
  /** `package.json` path for the given module. */
  packageJsonPath?: string
}

/**
 * Alias Value for [ResolveOptions::alias] and [ResolveOptions::fallback].
 * Use struct because napi don't support structured union now
 */
export interface Restriction {
  path?: string
  regex?: string
}

export declare function sync(path: string, request: string): ResolveResult

/**
 * Tsconfig Options
 *
 * Derived from [tsconfig-paths-webpack-plugin](https://github.com/dividab/tsconfig-paths-webpack-plugin#options)
 */
export interface TsconfigOptions {
  /**
   * Allows you to specify where to find the TypeScript configuration file.
   * You may provide
   * * a relative path to the configuration file. It will be resolved relative to cwd.
   * * an absolute path to the configuration file.
   */
  configFile: string
  /**
   * Support for Typescript Project References.
   *
   * * `'auto'`: use the `references` field from tsconfig of `config_file`.
   * * `string[]`: manually provided relative or absolute path.
   */
  references?: 'auto' | string[]
}
export interface SourceMap {
  file?: string
  mappings: string
  names: Array<string>
  sourceRoot?: string
  sources: Array<string>
  sourcesContent?: Array<string>
  version: number
  x_google_ignoreList?: Array<number>
}
export interface ArrowFunctionsOptions {
  /**
   * This option enables the following:
   * * Wrap the generated function in .bind(this) and keeps uses of this inside the function as-is, instead of using a renamed this.
   * * Add a runtime check to ensure the functions are not instantiated.
   * * Add names to arrow functions.
   *
   * @default false
   */
  spec?: boolean
}

export interface CompilerAssumptions {
  ignoreFunctionLength?: boolean
  noDocumentAll?: boolean
  objectRestNoSymbols?: boolean
  pureGetters?: boolean
  /**
   * When using public class fields, assume that they don't shadow any getter in the current class,
   * in its subclasses or in its superclass. Thus, it's safe to assign them rather than using
   * `Object.defineProperty`.
   *
   * For example:
   *
   * Input:
   * ```js
   * class Test {
   *  field = 2;
   *
   *  static staticField = 3;
   * }
   * ```
   *
   * When `set_public_class_fields` is `true`, the output will be:
   * ```js
   * class Test {
   *  constructor() {
   *    this.field = 2;
   *  }
   * }
   * Test.staticField = 3;
   * ```
   *
   * Otherwise, the output will be:
   * ```js
   * import _defineProperty from "@oxc-project/runtime/helpers/defineProperty";
   * class Test {
   *   constructor() {
   *     _defineProperty(this, "field", 2);
   *   }
   * }
   * _defineProperty(Test, "staticField", 3);
   * ```
   *
   * NOTE: For TypeScript, if you wanted behavior is equivalent to `useDefineForClassFields: false`, you should
   * set both `set_public_class_fields` and [`crate::TypeScriptOptions::remove_class_fields_without_initializer`]
   * to `true`.
   */
  setPublicClassFields?: boolean
}

export interface DecoratorOptions {
  /**
   * Enables experimental support for decorators, which is a version of decorators that predates the TC39 standardization process.
   *
   * Decorators are a language feature which hasn’t yet been fully ratified into the JavaScript specification.
   * This means that the implementation version in TypeScript may differ from the implementation in JavaScript when it it decided by TC39.
   *
   * @see https://www.typescriptlang.org/tsconfig/#experimentalDecorators
   * @default false
   */
  legacy?: boolean
  /**
   * Enables emitting decorator metadata.
   *
   * This option the same as [emitDecoratorMetadata](https://www.typescriptlang.org/tsconfig/#emitDecoratorMetadata)
   * in TypeScript, and it only works when `legacy` is true.
   *
   * @see https://www.typescriptlang.org/tsconfig/#emitDecoratorMetadata
   * @default false
   */
  emitDecoratorMetadata?: boolean
}

export interface Es2015Options {
  /** Transform arrow functions into function expressions. */
  arrowFunction?: ArrowFunctionsOptions
}

export type HelperMode = /**
 * Runtime mode (default): Helper functions are imported from a runtime package.
 *
 * Example:
 *
 * ```js
 * import helperName from "@oxc-project/runtime/helpers/helperName";
 * helperName(...arguments);
 * ```
 */
'Runtime'|
/**
 * External mode: Helper functions are accessed from a global `babelHelpers` object.
 *
 * Example:
 *
 * ```js
 * babelHelpers.helperName(...arguments);
 * ```
 */
'External';

export interface Helpers {
  mode?: HelperMode
}

/** TypeScript Isolated Declarations for Standalone DTS Emit */
export declare function isolatedDeclaration(filename: string, sourceText: string, options?: IsolatedDeclarationsOptions | undefined | null): IsolatedDeclarationsResult

export interface IsolatedDeclarationsOptions {
  /**
   * Do not emit declarations for code that has an @internal annotation in its JSDoc comment.
   * This is an internal compiler option; use at your own risk, because the compiler does not check that the result is valid.
   *
   * Default: `false`
   *
   * See <https://www.typescriptlang.org/tsconfig/#stripInternal>
   */
  stripInternal?: boolean
  sourcemap?: boolean
}

export interface IsolatedDeclarationsResult {
  code: string
  map?: SourceMap
  errors: Array<OxcError>
}

/**
 * Configure how TSX and JSX are transformed.
 *
 * @see {@link https://babeljs.io/docs/babel-plugin-transform-react-jsx#options}
 */
export interface JsxOptions {
  /**
   * Decides which runtime to use.
   *
   * - 'automatic' - auto-import the correct JSX factories
   * - 'classic' - no auto-import
   *
   * @default 'automatic'
   */
  runtime?: 'classic' | 'automatic'
  /**
   * Emit development-specific information, such as `__source` and `__self`.
   *
   * @default false
   *
   * @see {@link https://babeljs.io/docs/babel-plugin-transform-react-jsx-development}
   */
  development?: boolean
  /**
   * Toggles whether or not to throw an error if an XML namespaced tag name
   * is used.
   *
   * Though the JSX spec allows this, it is disabled by default since React's
   * JSX does not currently have support for it.
   *
   * @default true
   */
  throwIfNamespace?: boolean
  /**
   * Enables `@babel/plugin-transform-react-pure-annotations`.
   *
   * It will mark top-level React method calls as pure for tree shaking.
   *
   * @see {@link https://babeljs.io/docs/en/babel-plugin-transform-react-pure-annotations}
   *
   * @default true
   */
  pure?: boolean
  /**
   * Replaces the import source when importing functions.
   *
   * @default 'react'
   */
  importSource?: string
  /**
   * Replace the function used when compiling JSX expressions. It should be a
   * qualified name (e.g. `React.createElement`) or an identifier (e.g.
   * `createElement`).
   *
   * Only used for `classic` {@link runtime}.
   *
   * @default 'React.createElement'
   */
  pragma?: string
  /**
   * Replace the component used when compiling JSX fragments. It should be a
   * valid JSX tag name.
   *
   * Only used for `classic` {@link runtime}.
   *
   * @default 'React.Fragment'
   */
  pragmaFrag?: string
  /**
   * When spreading props, use `Object.assign` directly instead of an extend helper.
   *
   * Only used for `classic` {@link runtime}.
   *
   * @default false
   */
  useBuiltIns?: boolean
  /**
   * When spreading props, use inline object with spread elements directly
   * instead of an extend helper or Object.assign.
   *
   * Only used for `classic` {@link runtime}.
   *
   * @default false
   */
  useSpread?: boolean
  /**
   * Enable React Fast Refresh .
   *
   * Conforms to the implementation in {@link https://github.com/facebook/react/tree/v18.3.1/packages/react-refresh}
   *
   * @default false
   */
  refresh?: boolean | ReactRefreshOptions
}

/**
 * Transform JavaScript code to a Vite Node runnable module.
 *
 * @param filename The name of the file being transformed.
 * @param sourceText the source code itself
 * @param options The options for the transformation. See {@link
 * ModuleRunnerTransformOptions} for more information.
 *
 * @returns an object containing the transformed code, source maps, and any
 * errors that occurred during parsing or transformation.
 *
 * @deprecated Only works for Vite.
 */
export declare function moduleRunnerTransform(filename: string, sourceText: string, options?: ModuleRunnerTransformOptions | undefined | null): ModuleRunnerTransformResult

export interface ModuleRunnerTransformOptions {
  /**
   * Enable source map generation.
   *
   * When `true`, the `sourceMap` field of transform result objects will be populated.
   *
   * @default false
   *
   * @see {@link SourceMap}
   */
  sourcemap?: boolean
}

export interface ModuleRunnerTransformResult {
  /**
   * The transformed code.
   *
   * If parsing failed, this will be an empty string.
   */
  code: string
  /**
   * The source map for the transformed code.
   *
   * This will be set if {@link TransformOptions#sourcemap} is `true`.
   */
  map?: SourceMap
  deps: Array<string>
  dynamicDeps: Array<string>
  /**
   * Parse and transformation errors.
   *
   * Oxc's parser recovers from common syntax errors, meaning that
   * transformed code may still be available even if there are errors in this
   * list.
   */
  errors: Array<OxcError>
}

export interface PluginsOptions {
  styledComponents?: StyledComponentsOptions
}

export interface ReactRefreshOptions {
  /**
   * Specify the identifier of the refresh registration variable.
   *
   * @default `$RefreshReg$`.
   */
  refreshReg?: string
  /**
   * Specify the identifier of the refresh signature variable.
   *
   * @default `$RefreshSig$`.
   */
  refreshSig?: string
  emitFullSignatures?: boolean
}

/**
 * Configure how styled-components are transformed.
 *
 * @see {@link https://styled-components.com/docs/tooling#babel-plugin}
 */
export interface StyledComponentsOptions {
  /**
   * Enhances the attached CSS class name on each component with richer output to help
   * identify your components in the DOM without React DevTools.
   *
   * @default true
   */
  displayName?: boolean
  /**
   * Controls whether the `displayName` of a component will be prefixed with the filename
   * to make the component name as unique as possible.
   *
   * @default true
   */
  fileName?: boolean
  /**
   * Adds a unique identifier to every styled component to avoid checksum mismatches
   * due to different class generation on the client and server during server-side rendering.
   *
   * @default true
   */
  ssr?: boolean
  /**
   * Transpiles styled-components tagged template literals to a smaller representation
   * than what Babel normally creates, helping to reduce bundle size.
   *
   * @default true
   */
  transpileTemplateLiterals?: boolean
  /**
   * Minifies CSS content by removing all whitespace and comments from your CSS,
   * keeping valuable bytes out of your bundles.
   *
   * @default true
   */
  minify?: boolean
  /**
   * Enables transformation of JSX `css` prop when using styled-components.
   *
   * **Note: This feature is not yet implemented in oxc.**
   *
   * @default true
   */
  cssProp?: boolean
  /**
   * Enables "pure annotation" to aid dead code elimination by bundlers.
   *
   * @default false
   */
  pure?: boolean
  /**
   * Adds a namespace prefix to component identifiers to ensure class names are unique.
   *
   * Example: With `namespace: "my-app"`, generates `componentId: "my-app__sc-3rfj0a-1"`
   */
  namespace?: string
  /**
   * List of file names that are considered meaningless for component naming purposes.
   *
   * When the `fileName` option is enabled and a component is in a file with a name
   * from this list, the directory name will be used instead of the file name for
   * the component's display name.
   *
   * @default `["index"]`
   */
  meaninglessFileNames?: Array<string>
  /**
   * Import paths to be considered as styled-components imports at the top level.
   *
   * **Note: This feature is not yet implemented in oxc.**
   */
  topLevelImportPaths?: Array<string>
}

/**
 * Transpile a JavaScript or TypeScript into a target ECMAScript version.
 *
 * @param filename The name of the file being transformed. If this is a
 * relative path, consider setting the {@link TransformOptions#cwd} option..
 * @param sourceText the source code itself
 * @param options The options for the transformation. See {@link
 * TransformOptions} for more information.
 *
 * @returns an object containing the transformed code, source maps, and any
 * errors that occurred during parsing or transformation.
 */
export declare function transform(filename: string, sourceText: string, options?: TransformOptions | undefined | null): TransformResult

/**
 * Options for transforming a JavaScript or TypeScript file.
 *
 * @see {@link transform}
 */
export interface TransformOptions {
  /** Treat the source text as `js`, `jsx`, `ts`, `tsx`, or `dts`. */
  lang?: 'js' | 'jsx' | 'ts' | 'tsx' | 'dts'
  /** Treat the source text as `script` or `module` code. */
  sourceType?: 'script' | 'module' | 'unambiguous' | undefined
  /**
   * The current working directory. Used to resolve relative paths in other
   * options.
   */
  cwd?: string
  /**
   * Enable source map generation.
   *
   * When `true`, the `sourceMap` field of transform result objects will be populated.
   *
   * @default false
   *
   * @see {@link SourceMap}
   */
  sourcemap?: boolean
  /** Set assumptions in order to produce smaller output. */
  assumptions?: CompilerAssumptions
  /** Configure how TypeScript is transformed. */
  typescript?: TypeScriptOptions
  /** Configure how TSX and JSX are transformed. */
  jsx?: 'preserve' | JsxOptions
  /**
   * Sets the target environment for the generated JavaScript.
   *
   * The lowest target is `es2015`.
   *
   * Example:
   *
   * * `'es2015'`
   * * `['es2020', 'chrome58', 'edge16', 'firefox57', 'node12', 'safari11']`
   *
   * @default `esnext` (No transformation)
   *
   * @see [esbuild#target](https://esbuild.github.io/api/#target)
   */
  target?: string | Array<string>
  /** Behaviour for runtime helpers. */
  helpers?: Helpers
  /** Define Plugin */
  define?: Record<string, string>
  /** Inject Plugin */
  inject?: Record<string, string | [string, string]>
  /** Decorator plugin */
  decorator?: DecoratorOptions
  /** Third-party plugins to use. */
  plugins?: PluginsOptions
}

export interface TransformResult {
  /**
   * The transformed code.
   *
   * If parsing failed, this will be an empty string.
   */
  code: string
  /**
   * The source map for the transformed code.
   *
   * This will be set if {@link TransformOptions#sourcemap} is `true`.
   */
  map?: SourceMap
  /**
   * The `.d.ts` declaration file for the transformed code. Declarations are
   * only generated if `declaration` is set to `true` and a TypeScript file
   * is provided.
   *
   * If parsing failed and `declaration` is set, this will be an empty string.
   *
   * @see {@link TypeScriptOptions#declaration}
   * @see [declaration tsconfig option](https://www.typescriptlang.org/tsconfig/#declaration)
   */
  declaration?: string
  /**
   * Declaration source map. Only generated if both
   * {@link TypeScriptOptions#declaration declaration} and
   * {@link TransformOptions#sourcemap sourcemap} are set to `true`.
   */
  declarationMap?: SourceMap
  /**
   * Helpers used.
   *
   * @internal
   *
   * Example:
   *
   * ```text
   * { "_objectSpread": "@oxc-project/runtime/helpers/objectSpread2" }
   * ```
   */
  helpersUsed: Record<string, string>
  /**
   * Parse and transformation errors.
   *
   * Oxc's parser recovers from common syntax errors, meaning that
   * transformed code may still be available even if there are errors in this
   * list.
   */
  errors: Array<OxcError>
}

export interface TypeScriptOptions {
  jsxPragma?: string
  jsxPragmaFrag?: string
  onlyRemoveTypeImports?: boolean
  allowNamespaces?: boolean
  /**
   * When enabled, type-only class fields are only removed if they are prefixed with the declare modifier:
   *
   * @deprecated
   *
   * Allowing `declare` fields is built-in support in Oxc without any option. If you want to remove class fields
   * without initializer, you can use `remove_class_fields_without_initializer: true` instead.
   */
  allowDeclareFields?: boolean
  /**
   * When enabled, class fields without initializers are removed.
   *
   * For example:
   * ```ts
   * class Foo {
   *    x: number;
   *    y: number = 0;
   * }
   * ```
   * // transform into
   * ```js
   * class Foo {
   *    x: number;
   * }
   * ```
   *
   * The option is used to align with the behavior of TypeScript's `useDefineForClassFields: false` option.
   * When you want to enable this, you also need to set [`crate::CompilerAssumptions::set_public_class_fields`]
   * to `true`. The `set_public_class_fields: true` + `remove_class_fields_without_initializer: true` is
   * equivalent to `useDefineForClassFields: false` in TypeScript.
   *
   * When `set_public_class_fields` is true and class-properties plugin is enabled, the above example transforms into:
   *
   * ```js
   * class Foo {
   *   constructor() {
   *     this.y = 0;
   *   }
   * }
   * ```
   *
   * Defaults to `false`.
   */
  removeClassFieldsWithoutInitializer?: boolean
  /**
   * Also generate a `.d.ts` declaration file for TypeScript files.
   *
   * The source file must be compliant with all
   * [`isolatedDeclarations`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-5.html#isolated-declarations)
   * requirements.
   *
   * @default false
   */
  declaration?: IsolatedDeclarationsOptions
  /**
   * Rewrite or remove TypeScript import/export declaration extensions.
   *
   * - When set to `rewrite`, it will change `.ts`, `.mts`, `.cts` extensions to `.js`, `.mjs`, `.cjs` respectively.
   * - When set to `remove`, it will remove `.ts`/`.mts`/`.cts`/`.tsx` extension entirely.
   * - When set to `true`, it's equivalent to `rewrite`.
   * - When set to `false` or omitted, no changes will be made to the extensions.
   *
   * @default false
   */
  rewriteImportExtensions?: 'rewrite' | 'remove' | boolean
}
export declare class BindingBundleEndEventData {
  output: string
  duration: number
  get result(): BindingBundlerImpl
}

export declare class BindingBundleErrorEventData {
  get result(): BindingBundlerImpl
  get error(): Array<Error | BindingError>
}

export declare class BindingBundler {
  constructor()
  createImpl(options: BindingBundlerOptions): BindingBundlerImpl
}

export declare class BindingBundlerImpl {
  write(): Promise<BindingOutputs>
  generate(): Promise<BindingOutputs>
  scan(): Promise<BindingOutputs>
  close(): Promise<void>
  get closed(): boolean
  getWatchFiles(): Promise<Array<string>>
  generateHmrPatch(changedFiles: Array<string>): Promise<BindingGenerateHmrPatchReturn>
  hmrInvalidate(caller: string, firstInvalidatedBy?: string | undefined | null): Promise<BindingHmrOutput>
}

export declare class BindingCallableBuiltinPlugin {
  constructor(plugin: BindingBuiltinPlugin)
  resolveId(id: string, importer?: string | undefined | null, options?: BindingHookJsResolveIdOptions | undefined | null): Promise<BindingHookJsResolveIdOutput | undefined | null>
  load(id: string): Promise<BindingHookJsLoadOutput | undefined | null>
  transform(code: string, id: string, options: BindingTransformHookExtraArgs): Promise<BindingHookTransformOutput | undefined | null>
  watchChange(path: string, event: BindingJsWatchChangeEvent): Promise<undefined>
}

export declare class BindingChunkingContext {
  getModuleInfo(moduleId: string): BindingModuleInfo | null
}

export declare class BindingDevEngine {
  constructor(options: BindingBundlerOptions, devOptions?: BindingDevOptions | undefined | null)
  run(): Promise<void>
  ensureCurrentBuildFinish(): Promise<void>
  ensureLatestBuild(): Promise<void>
  scheduleBuildIfStale(): Promise<ScheduledBuild | null>
  invalidate(caller: string, firstInvalidatedBy?: string | undefined | null): Promise<BindingHmrUpdate>
}

export declare class BindingHmrOutput {
  get patch(): BindingHmrUpdate | null
  get errors(): Array<Error | BindingError>
}

export declare class BindingModuleInfo {
  id: string
  importers: Array<string>
  dynamicImporters: Array<string>
  importedIds: Array<string>
  dynamicallyImportedIds: Array<string>
  exports: Array<string>
  isEntry: boolean
  get code(): string | null
}

export declare class BindingNormalizedOptions {
  get input(): Array<string> | Record<string, string>
  get cwd(): string | null
  get platform(): 'node' | 'browser' | 'neutral'
  get shimMissingExports(): boolean
  get name(): string | null
  get cssEntryFilenames(): string | undefined
  get cssChunkFilenames(): string | undefined
  get entryFilenames(): string | undefined
  get chunkFilenames(): string | undefined
  get assetFilenames(): string | undefined
  get dir(): string | null
  get file(): string | null
  get format(): 'es' | 'cjs' | 'iife' | 'umd'
  get exports(): 'default' | 'named' | 'none' | 'auto'
  get esModule(): boolean | 'if-default-prop'
  get inlineDynamicImports(): boolean
  get sourcemap(): boolean | 'inline' | 'hidden'
  get sourcemapBaseUrl(): string | null
  get banner(): string | undefined | null | undefined
  get footer(): string | undefined | null | undefined
  get intro(): string | undefined | null | undefined
  get outro(): string | undefined | null | undefined
  get externalLiveBindings(): boolean
  get extend(): boolean
  get globals(): Record<string, string> | undefined
  get hashCharacters(): 'base64' | 'base36' | 'hex'
  get sourcemapDebugIds(): boolean
  get polyfillRequire(): boolean
  get minify(): false | 'dce-only' | MinifyOptions
  get legalComments(): 'none' | 'inline'
  get preserveModules(): boolean
  get preserveModulesRoot(): string | undefined
  get virtualDirname(): string
  get topLevelVar(): boolean
  get minifyInternalExports(): boolean
  get context(): string
}

export declare class BindingOutputAsset {
  get fileName(): string
  get originalFileName(): string | null
  get originalFileNames(): Array<string>
  get source(): BindingAssetSource
  get name(): string | null
  get names(): Array<string>
}

export declare class BindingOutputChunk {
  get isEntry(): boolean
  get isDynamicEntry(): boolean
  get facadeModuleId(): string | null
  get moduleIds(): Array<string>
  get exports(): Array<string>
  get fileName(): string
  get modules(): BindingModules
  get imports(): Array<string>
  get dynamicImports(): Array<string>
  get code(): string
  get map(): string | null
  get sourcemapFileName(): string | null
  get preliminaryFileName(): string
  get name(): string
}

export declare class BindingOutputs {
  get chunks(): Array<BindingOutputChunk>
  get assets(): Array<BindingOutputAsset>
  get errors(): Array<Error | BindingError>
}

export declare class BindingPluginContext {
  load(specifier: string, sideEffects: boolean | 'no-treeshake' | undefined): Promise<void>
  resolve(specifier: string, importer?: string | undefined | null, extraOptions?: BindingPluginContextResolveOptions | undefined | null): Promise<BindingPluginContextResolvedId | null>
  emitFile(file: BindingEmittedAsset, assetFilename?: string | undefined | null, fnSanitizedFileName?: string | undefined | null): string
  emitChunk(file: BindingEmittedChunk): string
  getFileName(referenceId: string): string
  getModuleInfo(moduleId: string): BindingModuleInfo | null
  getModuleIds(): Array<string>
  addWatchFile(file: string): void
}

export declare class BindingRenderedChunk {
  get name(): string
  get isEntry(): boolean
  get isDynamicEntry(): boolean
  get facadeModuleId(): string | null
  get moduleIds(): Array<string>
  get exports(): Array<string>
  get fileName(): string
  get modules(): BindingModules
  get imports(): Array<string>
  get dynamicImports(): Array<string>
}

export declare class BindingRenderedChunkMeta {
  get chunks(): Record<string, BindingRenderedChunk>
}

export declare class BindingRenderedModule {
  get code(): string | null
  get renderedExports(): Array<string>
}

export declare class BindingTransformPluginContext {
  getCombinedSourcemap(): string
  inner(): BindingPluginContext
  addWatchFile(file: string): void
}

export declare class BindingWatcher {
  constructor(options: Array<BindingBundlerOptions>, notifyOption?: BindingNotifyOption | undefined | null)
  close(): Promise<void>
  start(listener: (data: BindingWatcherEvent) => void): Promise<void>
}

export declare class BindingWatcherChangeData {
  path: string
  kind: string
}

export declare class BindingWatcherEvent {
  eventKind(): string
  watchChangeData(): BindingWatcherChangeData
  bundleEndData(): BindingBundleEndEventData
  bundleEventKind(): string
  bundleErrorData(): BindingBundleErrorEventData
}

export declare class ParallelJsPluginRegistry {
  id: number
  workerCount: number
  constructor(workerCount: number)
}

export declare class ScheduledBuild {
  wait(): Promise<void>
  alreadyScheduled(): boolean
}

export declare class TraceSubscriberGuard {
  close(): void
}

export interface AliasItem {
  find: string
  replacements: Array<string>
}

export interface BindingAdvancedChunksOptions {
  includeDependenciesRecursively?: boolean
  minSize?: number
  minShareCount?: number
  groups?: Array<BindingMatchGroup>
  maxSize?: number
  minModuleSize?: number
  maxModuleSize?: number
}

export interface BindingAliasPluginAlias {
  find: BindingStringOrRegex
  replacement: string
}

export interface BindingAliasPluginConfig {
  entries: Array<BindingAliasPluginAlias>
}

export interface BindingAssetPluginConfig {
  isLib?: boolean
  isSsr?: boolean
  isWorker?: boolean
  urlBase?: string
  publicDir?: string
  decodedBase?: string
  isSkipAssets?: boolean
  assetsInclude?: Array<BindingStringOrRegex>
  assetInlineLimit?: number | ((file: string, content: Buffer) => boolean | undefined)
  renderBuiltUrl?: (filename: string, type: BindingRenderBuiltUrlConfig) => MaybePromise<VoidNullable<string | BindingRenderBuiltUrlRet>>
}

export interface BindingAssetSource {
  inner: string | Uint8Array
}

export declare enum BindingAttachDebugInfo {
  None = 0,
  Simple = 1,
  Full = 2
}

export interface BindingBuildImportAnalysisPluginConfig {
  preloadCode: string
  insertPreload: boolean
  optimizeModulePreloadRelativePaths: boolean
  renderBuiltUrl: boolean
  isRelativeBase: boolean
}

export interface BindingBuiltinPlugin {
  __name: BindingBuiltinPluginName
  options?: unknown
}

export type BindingBuiltinPluginName =  'builtin:alias'|
'builtin:asset'|
'builtin:asset-import-meta-url'|
'builtin:build-import-analysis'|
'builtin:dynamic-import-vars'|
'builtin:import-glob'|
'builtin:isolated-declaration'|
'builtin:json'|
'builtin:load-fallback'|
'builtin:manifest'|
'builtin:module-preload-polyfill'|
'builtin:oxc-runtime'|
'builtin:reporter'|
'builtin:replace'|
'builtin:esm-external-require'|
'builtin:transform'|
'builtin:vite-resolve'|
'builtin:wasm-fallback'|
'builtin:wasm-helper'|
'builtin:web-worker-post';

export interface BindingBundlerOptions {
  inputOptions: BindingInputOptions
  outputOptions: BindingOutputOptions
  parallelPluginsRegistry?: ParallelJsPluginRegistry
}

export interface BindingChecksOptions {
  circularDependency?: boolean
  eval?: boolean
  missingGlobalName?: boolean
  missingNameOptionForIifeExport?: boolean
  mixedExport?: boolean
  unresolvedEntry?: boolean
  unresolvedImport?: boolean
  filenameConflict?: boolean
  commonJsVariableInEsm?: boolean
  importIsUndefined?: boolean
  emptyImportMeta?: boolean
  configurationFieldConflict?: boolean
  preferBuiltinFeature?: boolean
}

export interface BindingChunkImportMap {
  baseUrl?: string
  fileName?: string
}

export declare enum BindingChunkModuleOrderBy {
  ModuleId = 0,
  ExecOrder = 1
}

export interface BindingDebugOptions {
  sessionId?: string
}

export interface BindingDeferSyncScanData {
  /** ModuleId */
  id: string
  sideEffects?: boolean | 'no-treeshake'
}

export interface BindingDevOptions {
  onHmrUpdates?: undefined | ((updates: BindingHmrUpdate[], changedFiles: string[]) => void | Promise<void>)
  watch?: BindingDevWatchOptions
}

export interface BindingDevWatchOptions {
  usePolling?: boolean
  pollInterval?: number
  useDebounce?: boolean
  debounceDuration?: number
  compareContentsForPolling?: boolean
  debounceTickRate?: number
}

export interface BindingDynamicImportVarsPluginConfig {
  include?: Array<BindingStringOrRegex>
  exclude?: Array<BindingStringOrRegex>
  resolver?: (id: string, importer: string) => MaybePromise<string | undefined>
}

export interface BindingEmittedAsset {
  name?: string
  fileName?: string
  originalFileName?: string
  source: BindingAssetSource
}

export interface BindingEmittedChunk {
  name?: string
  fileName?: string
  id: string
  importer?: string
  preserveEntrySignatures?: BindingPreserveEntrySignatures
}

export interface BindingError {
  kind: string
  message: string
}

export interface BindingEsmExternalRequirePluginConfig {
  external: Array<BindingStringOrRegex>
}

export interface BindingExperimentalHmrOptions {
  host?: string
  port?: number
  implement?: string
  new?: boolean
}

export interface BindingExperimentalOptions {
  strictExecutionOrder?: boolean
  disableLiveBindings?: boolean
  viteMode?: boolean
  resolveNewUrlToAsset?: boolean
  hmr?: BindingExperimentalHmrOptions
  attachDebugInfo?: BindingAttachDebugInfo
  chunkModulesOrder?: BindingChunkModuleOrderBy
  chunkImportMap?: boolean | BindingChunkImportMap
  onDemandWrapping?: boolean
  incrementalBuild?: boolean
  transformHiresSourcemap?: boolean | 'boundary'
}

export interface BindingFilterToken {
  kind: FilterTokenKind
  payload?: BindingStringOrRegex | number | boolean
}

export type BindingGenerateHmrPatchReturn =
  | { type: 'Ok', field0: Array<BindingHmrUpdate> }
  | { type: 'Error', field0: Array<Error | BindingError> }

export interface BindingHmrBoundaryOutput {
  boundary: string
  acceptedVia: string
}

export type BindingHmrUpdate =
  | { type: 'Patch', code: string, filename: string, sourcemap?: string, sourcemapFilename?: string, hmrBoundaries: Array<BindingHmrBoundaryOutput> }
  | { type: 'FullReload', reason?: string }
  | { type: 'Noop' }

export interface BindingHookFilter {
  value?: Array<Array<BindingFilterToken>>
}

export interface BindingHookJsLoadOutput {
  code: string
  map?: string
  moduleSideEffects?: boolean | 'no-treeshake'
}

export interface BindingHookJsResolveIdOptions {
  isEntry?: boolean
  scan?: boolean
  custom?: BindingVitePluginCustom
}

export interface BindingHookJsResolveIdOutput {
  id: string
  external?: boolean | 'absolute' | 'relative'
  moduleSideEffects?: boolean | 'no-treeshake'
}

export interface BindingHookLoadOutput {
  code: string
  moduleSideEffects?: boolean | 'no-treeshake'
  map?: BindingSourcemap
  moduleType?: string
}

export interface BindingHookRenderChunkOutput {
  code: string
  map?: BindingSourcemap
}

export interface BindingHookResolveIdExtraArgs {
  custom?: number
  isEntry: boolean
  /**
   * - `import-statement`: `import { foo } from './lib.js';`
   * - `dynamic-import`: `import('./lib.js')`
   * - `require-call`: `require('./lib.js')`
   * - `import-rule`: `@import 'bg-color.css'`
   * - `url-token`: `url('./icon.png')`
   * - `new-url`: `new URL('./worker.js', import.meta.url)`
   * - `hot-accept`: `import.meta.hot.accept('./lib.js', () => {})`
   */
  kind: 'import-statement' | 'dynamic-import' | 'require-call' | 'import-rule' | 'url-token' | 'new-url' | 'hot-accept'
}

export interface BindingHookResolveIdOutput {
  id: string
  external?: BindingResolvedExternal
  normalizeExternalId?: boolean
  moduleSideEffects?: boolean | 'no-treeshake'
  /**
   * @internal Used to store package json path resolved by oxc resolver,
   * we could get the related package json object via the path string.
   */
  packageJsonPath?: string | null
}

export type BindingHookSideEffects =
  boolean | string

export interface BindingHookTransformOutput {
  code?: string
  moduleSideEffects?: BindingHookSideEffects
  map?: BindingSourcemap
  moduleType?: string
}

export interface BindingImportGlobPluginConfig {
  root?: string
  restoreQueryExtension?: boolean
}

export interface BindingInjectImportNamed {
  tagNamed: true
  imported: string
  alias?: string
  from: string
}

export interface BindingInjectImportNamespace {
  tagNamespace: true
  alias: string
  from: string
}

export interface BindingInlineConstConfig {
  mode?: string
  pass?: number
}

export interface BindingInputItem {
  name?: string
  import: string
}

export interface BindingInputOptions {
  external?: undefined | ((source: string, importer: string | undefined, isResolved: boolean) => boolean)
  input: Array<BindingInputItem>
  plugins: (BindingBuiltinPlugin | BindingPluginOptions | undefined)[]
  resolve?: BindingResolveOptions
  shimMissingExports?: boolean
  platform?: 'node' | 'browser' | 'neutral'
  logLevel: BindingLogLevel
  onLog: (logLevel: 'debug' | 'warn' | 'info', log: BindingLog) => Promise<void>
  cwd: string
  treeshake?: BindingTreeshake
  moduleTypes?: Record<string, string>
  define?: Array<[string, string]>
  dropLabels?: Array<string>
  inject?: Array<BindingInjectImportNamed | BindingInjectImportNamespace>
  experimental?: BindingExperimentalOptions
  profilerNames?: boolean
  jsx?: BindingJsx
  transform?: TransformOptions
  watch?: BindingWatchOption
  keepNames?: boolean
  checks?: BindingChecksOptions
  deferSyncScanData?: undefined | (() => BindingDeferSyncScanData[])
  makeAbsoluteExternalsRelative?: BindingMakeAbsoluteExternalsRelative
  debug?: BindingDebugOptions
  invalidateJsSideCache?: () => void
  markModuleLoaded?: (id: string, success: boolean) => void
  preserveEntrySignatures?: BindingPreserveEntrySignatures
  optimization?: BindingOptimization
  context?: string
  tsconfig?: string
}

export interface BindingIsolatedDeclarationPluginConfig {
  stripInternal?: boolean
}

export interface BindingJsonPluginConfig {
  minify?: boolean
  namedExports?: boolean
  stringify?: BindingJsonPluginStringify
}

export type BindingJsonPluginStringify =
  boolean | string

export interface BindingJsonSourcemap {
  file?: string
  mappings?: string
  sourceRoot?: string
  sources?: Array<string | undefined | null>
  sourcesContent?: Array<string | undefined | null>
  names?: Array<string>
  debugId?: string
  x_google_ignoreList?: Array<number>
}

export interface BindingJsWatchChangeEvent {
  event: string
}

export declare enum BindingJsx {
  Disable = 0,
  Preserve = 1,
  React = 2,
  ReactJsx = 3
}

export interface BindingLog {
  message: string
  id?: string
  code?: string
  exporter?: string
  plugin?: string
}

export declare enum BindingLogLevel {
  Silent = 0,
  Warn = 1,
  Info = 2,
  Debug = 3
}

export type BindingMakeAbsoluteExternalsRelative =
  | { type: 'Bool', field0: boolean }
  | { type: 'IfRelativeSource' }

export interface BindingManifestPluginConfig {
  root: string
  outPath: string
  isLegacy?: () => boolean
  cssEntries: () => Set<string>
}

export interface BindingMatchGroup {
  name: string | ((id: string, ctx: BindingChunkingContext) => VoidNullable<string>)
  test?: string | RegExp | ((id: string) => VoidNullable<boolean>)
  priority?: number
  minSize?: number
  minShareCount?: number
  minModuleSize?: number
  maxModuleSize?: number
  maxSize?: number
}

export interface BindingModulePreloadPolyfillPluginConfig {
  isServer?: boolean
}

export interface BindingModules {
  values: Array<BindingRenderedModule>
  keys: Array<string>
}

export interface BindingModuleSideEffectsRule {
  test?: RegExp | undefined
  sideEffects: boolean
  external?: boolean | undefined
}

export interface BindingNotifyOption {
  pollInterval?: number
  compareContents?: boolean
}

export interface BindingOptimization {
  inlineConst?: boolean | BindingInlineConstConfig
  pifeForModuleWrappers?: boolean
}

export interface BindingOutputOptions {
  name?: string
  assetFileNames?: string | ((chunk: BindingPreRenderedAsset) => string)
  entryFileNames?: string | ((chunk: PreRenderedChunk) => string)
  chunkFileNames?: string | ((chunk: PreRenderedChunk) => string)
  cssEntryFileNames?: string | ((chunk: PreRenderedChunk) => string)
  cssChunkFileNames?: string | ((chunk: PreRenderedChunk) => string)
  sanitizeFileName?: boolean | ((name: string) => string)
  banner?: (chunk: BindingRenderedChunk) => MaybePromise<VoidNullable<string>>
  dir?: string
  file?: string
  esModule?: boolean | 'if-default-prop'
  exports?: 'default' | 'named' | 'none' | 'auto'
  extend?: boolean
  externalLiveBindings?: boolean
  footer?: (chunk: BindingRenderedChunk) => MaybePromise<VoidNullable<string>>
  format?: 'es' | 'cjs' | 'iife' | 'umd'
  globals?: Record<string, string> | ((name: string) => string)
  hashCharacters?: 'base64' | 'base36' | 'hex'
  inlineDynamicImports?: boolean
  intro?: (chunk: BindingRenderedChunk) => MaybePromise<VoidNullable<string>>
  outro?: (chunk: BindingRenderedChunk) => MaybePromise<VoidNullable<string>>
  plugins: (BindingBuiltinPlugin | BindingPluginOptions | undefined)[]
  sourcemap?: 'file' | 'inline' | 'hidden'
  sourcemapBaseUrl?: string
  sourcemapIgnoreList?: (source: string, sourcemapPath: string) => boolean
  sourcemapDebugIds?: boolean
  sourcemapPathTransform?: (source: string, sourcemapPath: string) => string
  minify?: boolean | 'dce-only' | MinifyOptions
  advancedChunks?: BindingAdvancedChunksOptions
  legalComments?: 'none' | 'inline'
  polyfillRequire?: boolean
  preserveModules?: boolean
  virtualDirname?: string
  preserveModulesRoot?: string
  topLevelVar?: boolean
  minifyInternalExports?: boolean
}

export interface BindingOxcRuntimePluginConfig {
  resolveBase?: string
}

export interface BindingPluginContextResolvedId {
  id: string
  packageJsonPath?: string
  external: boolean | 'absolute' | 'relative'
  moduleSideEffects?: boolean | 'no-treeshake'
}

export interface BindingPluginContextResolveOptions {
  /**
   * - `import-statement`: `import { foo } from './lib.js';`
   * - `dynamic-import`: `import('./lib.js')`
   * - `require-call`: `require('./lib.js')`
   * - `import-rule`: `@import 'bg-color.css'`
   * - `url-token`: `url('./icon.png')`
   * - `new-url`: `new URL('./worker.js', import.meta.url)`
   * - `hot-accept`: `import.meta.hot.accept('./lib.js', () => {})`
   */
  importKind?: 'import-statement' | 'dynamic-import' | 'require-call' | 'import-rule' | 'url-token' | 'new-url' | 'hot-accept'
  isEntry?: boolean
  skipSelf?: boolean
  custom?: number
  vitePluginCustom?: BindingVitePluginCustom
}

export interface BindingPluginHookMeta {
  order?: BindingPluginOrder
}

export interface BindingPluginOptions {
  name: string
  hookUsage: number
  buildStart?: (ctx: BindingPluginContext, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable>
  buildStartMeta?: BindingPluginHookMeta
  resolveId?: (ctx: BindingPluginContext, specifier: string, importer: Nullable<string>, options: BindingHookResolveIdExtraArgs) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  resolveIdMeta?: BindingPluginHookMeta
  resolveIdFilter?: BindingHookFilter
  resolveDynamicImport?: (ctx: BindingPluginContext, specifier: string, importer: Nullable<string>) => MaybePromise<VoidNullable<BindingHookResolveIdOutput>>
  resolveDynamicImportMeta?: BindingPluginHookMeta
  load?: (ctx: BindingPluginContext, id: string) => MaybePromise<VoidNullable<BindingHookLoadOutput>>
  loadMeta?: BindingPluginHookMeta
  loadFilter?: BindingHookFilter
  transform?: (ctx:  BindingTransformPluginContext, id: string, code: string, module_type: BindingTransformHookExtraArgs) => MaybePromise<VoidNullable<BindingHookTransformOutput>>
  transformMeta?: BindingPluginHookMeta
  transformFilter?: BindingHookFilter
  moduleParsed?: (ctx: BindingPluginContext, module: BindingModuleInfo) => MaybePromise<VoidNullable>
  moduleParsedMeta?: BindingPluginHookMeta
  buildEnd?: (ctx: BindingPluginContext, error?: (Error | BindingError)[]) => MaybePromise<VoidNullable>
  buildEndMeta?: BindingPluginHookMeta
  renderChunk?: (ctx: BindingPluginContext, code: string, chunk: BindingRenderedChunk, opts: BindingNormalizedOptions, meta: BindingRenderedChunkMeta) => MaybePromise<VoidNullable<BindingHookRenderChunkOutput>>
  renderChunkMeta?: BindingPluginHookMeta
  renderChunkFilter?: BindingHookFilter
  augmentChunkHash?: (ctx: BindingPluginContext, chunk: BindingRenderedChunk) => MaybePromise<void | string>
  augmentChunkHashMeta?: BindingPluginHookMeta
  renderStart?: (ctx: BindingPluginContext, opts: BindingNormalizedOptions) => void
  renderStartMeta?: BindingPluginHookMeta
  renderError?: (ctx: BindingPluginContext, error: (Error | BindingError)[]) => void
  renderErrorMeta?: BindingPluginHookMeta
  generateBundle?: (ctx: BindingPluginContext, bundle: BindingOutputs, isWrite: boolean, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable<JsChangedOutputs>>
  generateBundleMeta?: BindingPluginHookMeta
  writeBundle?: (ctx: BindingPluginContext, bundle: BindingOutputs, opts: BindingNormalizedOptions) => MaybePromise<VoidNullable<JsChangedOutputs>>
  writeBundleMeta?: BindingPluginHookMeta
  closeBundle?: (ctx: BindingPluginContext) => MaybePromise<VoidNullable>
  closeBundleMeta?: BindingPluginHookMeta
  watchChange?: (ctx: BindingPluginContext, path: string, event: string) => MaybePromise<VoidNullable>
  watchChangeMeta?: BindingPluginHookMeta
  closeWatcher?: (ctx: BindingPluginContext) => MaybePromise<VoidNullable>
  closeWatcherMeta?: BindingPluginHookMeta
  banner?: (ctx: BindingPluginContext, chunk: BindingRenderedChunk) => void
  bannerMeta?: BindingPluginHookMeta
  footer?: (ctx: BindingPluginContext, chunk: BindingRenderedChunk) => void
  footerMeta?: BindingPluginHookMeta
  intro?: (ctx: BindingPluginContext, chunk: BindingRenderedChunk) => void
  introMeta?: BindingPluginHookMeta
  outro?: (ctx: BindingPluginContext, chunk: BindingRenderedChunk) => void
  outroMeta?: BindingPluginHookMeta
}

export declare enum BindingPluginOrder {
  Pre = 0,
  Post = 1
}

export interface BindingPluginWithIndex {
  index: number
  plugin: BindingPluginOptions
}

export interface BindingPreRenderedAsset {
  name?: string
  names: Array<string>
  originalFileName?: string
  originalFileNames: Array<string>
  source: BindingAssetSource
}

export type BindingPreserveEntrySignatures =
  | { type: 'Bool', field0: boolean }
  | { type: 'String', field0: string }

export declare enum BindingPropertyReadSideEffects {
  Always = 0,
  False = 1
}

export declare enum BindingPropertyWriteSideEffects {
  Always = 0,
  False = 1
}

export interface BindingRenderBuiltUrlConfig {
  ssr: boolean
  type: 'asset' | 'public'
  hostId: string
  hostType: 'js' | 'css' | 'html'
}

export interface BindingRenderBuiltUrlRet {
  relative?: boolean
  runtime?: string
}

export interface BindingReplacePluginConfig {
  values: Record<string, string>
  delimiters?: [string, string]
  preventAssignment?: boolean
  objectGuards?: boolean
  sourcemap?: boolean
}

export interface BindingReporterPluginConfig {
  isTty: boolean
  isLib: boolean
  assetsDir: string
  chunkLimit: number
  shouldLogInfo: boolean
  warnLargeChunks: boolean
  reportCompressedSize: boolean
}

export type BindingResolvedExternal =
  boolean | string

export interface BindingResolveOptions {
  alias?: Array<AliasItem>
  aliasFields?: Array<Array<string>>
  conditionNames?: Array<string>
  exportsFields?: Array<Array<string>>
  extensions?: Array<string>
  extensionAlias?: Array<ExtensionAliasItem>
  mainFields?: Array<string>
  mainFiles?: Array<string>
  modules?: Array<string>
  symlinks?: boolean
  yarnPnp?: boolean
}

export interface BindingSourcemap {
  inner: string | BindingJsonSourcemap
}

export interface BindingTransformHookExtraArgs {
  moduleType: string
}

export interface BindingTransformPluginConfig {
  include?: Array<BindingStringOrRegex>
  exclude?: Array<BindingStringOrRegex>
  jsxRefreshInclude?: Array<BindingStringOrRegex>
  jsxRefreshExclude?: Array<BindingStringOrRegex>
  isServerConsumer?: boolean
  runtimeResolveBase?: string
  jsxInject?: string
  transformOptions?: TransformOptions
}

export interface BindingTreeshake {
  moduleSideEffects: boolean | ReadonlyArray<string> | BindingModuleSideEffectsRule[] | ((id: string, external: boolean) => boolean | undefined)
  annotations?: boolean
  manualPureFunctions?: ReadonlyArray<string>
  unknownGlobalSideEffects?: boolean
  commonjs?: boolean
  propertyReadSideEffects?: BindingPropertyReadSideEffects
  propertyWriteSideEffects?: BindingPropertyWriteSideEffects
}

export interface BindingVitePluginCustom {
  'vite:import-glob'?: ViteImportGlobMeta
}

export interface BindingViteResolvePluginConfig {
  resolveOptions: BindingViteResolvePluginResolveOptions
  environmentConsumer: string
  environmentName: string
  builtins: Array<BindingStringOrRegex>
  external: true | string[]
  noExternal: true | Array<string | RegExp>
  dedupe: Array<string>
  finalizeBareSpecifier?: (resolvedId: string, rawId: string, importer: string | null | undefined) => VoidNullable<string>
  finalizeOtherSpecifiers?: (resolvedId: string, rawId: string) => VoidNullable<string>
  resolveSubpathImports: (id: string, importer: string, isRequire: boolean, scan: boolean) => VoidNullable<string>
  onWarn?: (message: string) => void
  onDebug?: (message: string) => void
}

export interface BindingViteResolvePluginResolveOptions {
  isBuild: boolean
  isProduction: boolean
  asSrc: boolean
  preferRelative: boolean
  isRequire?: boolean
  root: string
  scan: boolean
  mainFields: Array<string>
  conditions: Array<string>
  externalConditions: Array<string>
  extensions: Array<string>
  tryIndex: boolean
  tryPrefix?: string
  preserveSymlinks: boolean
  tsconfigPaths: boolean
}

export interface BindingWasmHelperPluginConfig {
  decodedBase: string
}

export interface BindingWatchOption {
  skipWrite?: boolean
  include?: Array<BindingStringOrRegex>
  exclude?: Array<BindingStringOrRegex>
  buildDelay?: number
  onInvalidate?: ((id: string) => void) | undefined
}

export interface ExtensionAliasItem {
  target: string
  replacements: Array<string>
}

export type FilterTokenKind =  'Id'|
'Code'|
'ModuleType'|
'And'|
'Or'|
'Not'|
'Include'|
'Exclude'|
'CleanUrl'|
'QueryKey'|
'QueryValue';

export declare function initTraceSubscriber(): TraceSubscriberGuard | null

export interface JsChangedOutputs {
  deleted: Set<string>
  changes: Record<string, JsOutputChunk | JsOutputAsset>
}

export interface JsOutputAsset {
  names: Array<string>
  originalFileNames: Array<string>
  filename: string
  source: BindingAssetSource
}

export interface JsOutputChunk {
  name: string
  isEntry: boolean
  isDynamicEntry: boolean
  facadeModuleId?: string
  moduleIds: Array<string>
  exports: Array<string>
  filename: string
  modules: Record<string, BindingRenderedModule>
  imports: Array<string>
  dynamicImports: Array<string>
  code: string
  map?: BindingSourcemap
  sourcemapFilename?: string
  preliminaryFilename: string
}

export interface PreRenderedChunk {
  name: string
  isEntry: boolean
  isDynamicEntry: boolean
  facadeModuleId?: string
  moduleIds: Array<string>
  exports: Array<string>
}

export declare function registerPlugins(id: number, plugins: Array<BindingPluginWithIndex>): void

/**
 * Shutdown the tokio runtime manually.
 *
 * This is required for the wasm target with `tokio_unstable` cfg.
 * In the wasm runtime, the `park` threads will hang there until the tokio::Runtime is shutdown.
 */
export declare function shutdownAsyncRuntime(): void

/**
 * Start the async runtime manually.
 *
 * This is required when the async runtime is shutdown manually.
 * Usually it's used in test.
 */
export declare function startAsyncRuntime(): void

export interface ViteImportGlobMeta {
  isSubImportsPattern?: boolean
}
export declare class JsWatcher {
  constructor(options: JsWatcherOptions)
}

export interface JsWatcherOptions {
  watch: ((err: Error | null, arg: string) => any)
  unwatch: ((err: Error | null, arg: string) => any)
}
