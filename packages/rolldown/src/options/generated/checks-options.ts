// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/checks.rs`

export interface ChecksOptions {
  /**
   * Whether to emit warnings when detecting circular dependency
   *
   * Circular dependencies lead to a bigger bundle size and sometimes cause execution order issues and are better to avoid.
   * @default false
   * */
  circularDependency?: boolean;

  /**
   * Whether to emit warnings when detecting uses of direct `eval`s
   *
   * See [Avoiding Direct `eval` in Troubleshooting page](https://rolldown.rs/guide/troubleshooting#avoiding-direct-eval) for more details.
   * @default true
   * */
  eval?: boolean;

  /**
   * Whether to emit warnings when the `output.globals` option is missing when needed
   *
   * See [`output.globals`](https://rolldown.rs/reference/OutputOptions.globals).
   * @default true
   * */
  missingGlobalName?: boolean;

  /**
   * Whether to emit warnings when the `output.name` option is missing when needed
   *
   * See [`output.name`](https://rolldown.rs/reference/OutputOptions.name).
   * @default true
   * */
  missingNameOptionForIifeExport?: boolean;

  /**
   * Whether to emit warnings when the way to export values is ambiguous
   *
   * See [`output.exports`](https://rolldown.rs/reference/OutputOptions.exports).
   * @default true
   * */
  mixedExports?: boolean;

  /**
   * Whether to emit warnings when an entrypoint cannot be resolved
   * @default true
   * */
  unresolvedEntry?: boolean;

  /**
   * Whether to emit warnings when an import cannot be resolved
   * @default true
   * */
  unresolvedImport?: boolean;

  /**
   * Whether to emit warnings when files generated have the same name with different contents
   * @default true
   * */
  filenameConflict?: boolean;

  /**
   * Whether to emit warnings when a CommonJS variable is used in an ES module
   *
   * CommonJS variables like `module` and `exports` are treated as global variables in ES modules and may not work as expected.
   * @default true
   * */
  commonJsVariableInEsm?: boolean;

  /**
   * Whether to emit warnings when an imported variable is not exported
   *
   * If the code is importing a variable that is not exported by the imported module, the value will always be `undefined`. This might be a mistake in the code.
   * @default true
   * */
  importIsUndefined?: boolean;

  /**
   * Whether to emit warnings when `import.meta` is not supported with the output format and is replaced with an empty object (`{}`)
   *
   * See [`import.meta` in Non-ESM Output Formats page](https://rolldown.rs/in-depth/non-esm-output-formats#import-meta) for more details.
   * @default true
   * */
  emptyImportMeta?: boolean;

  /**
   * Whether to emit warnings when detecting tolerated transform
   * @default true
   * */
  toleratedTransform?: boolean;

  /**
   * Whether to emit warnings when a namespace is called as a function
   *
   * A module namespace object is an object and not a function. Calling it as a function will cause a runtime error.
   * @default true
   * */
  cannotCallNamespace?: boolean;

  /**
   * Whether to emit warnings when a config value is overridden by another config value with a higher priority
   * @default true
   * */
  configurationFieldConflict?: boolean;

  /**
   * Whether to emit warnings when a plugin that is covered by a built-in feature is used
   *
   * Using built-in features is generally more performant than using plugins.
   * @default true
   * */
  preferBuiltinFeature?: boolean;

  /**
   * Whether to emit warnings when Rolldown could not clean the output directory
   *
   * See [`output.cleanDir`](https://rolldown.rs/reference/OutputOptions.cleanDir).
   * @default true
   * */
  couldNotCleanDirectory?: boolean;

  /**
   * Whether to emit warnings when plugins take significant time during the build process
   *
   * {@include ../docs/checks-plugin-timings.md}
   * @default true
   * */
  pluginTimings?: boolean;
}
