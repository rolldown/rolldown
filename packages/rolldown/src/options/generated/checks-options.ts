// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/checks.rs`

export interface ChecksOptions {
  /**
   * Whether to emit warnings when detecting circular dependency.
   *
   * Circular dependencies lead to a bigger bundle size and sometimes cause execution order issues and are better to avoid.
   *
   * {@include ../docs/checks-circular-dependency.md}
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default false
   * */
  circularDependency?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when detecting uses of direct `eval`s.
   *
   * See [Avoiding Direct `eval` in Troubleshooting page](https://rolldown.rs/guide/troubleshooting#avoiding-direct-eval) for more details.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  eval?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when the `output.globals` option is missing when needed.
   *
   * See [`output.globals`](https://rolldown.rs/reference/OutputOptions.globals).
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  missingGlobalName?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when the `output.name` option is missing when needed.
   *
   * See [`output.name`](https://rolldown.rs/reference/OutputOptions.name).
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  missingNameOptionForIifeExport?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when a `#__PURE__` / `@__PURE__` annotation has no effect due to its position.
   *
   * Annotations placed where they cannot annotate a call expression (e.g. before a non-call expression,
   * before a statement declaration, or between an identifier and `=` in a variable declarator) are
   * ignored by the parser. Matches Rollup's `INVALID_ANNOTATION` log code.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  invalidAnnotation?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when the way to export values is ambiguous.
   *
   * See [`output.exports`](https://rolldown.rs/reference/OutputOptions.exports).
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  mixedExports?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when an entrypoint cannot be resolved.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  unresolvedEntry?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when an import cannot be resolved.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  unresolvedImport?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when files generated have the same name with different contents.
   *
   * {@include ../docs/checks-filename-conflict.md}
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  filenameConflict?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when a CommonJS variable is used in an ES module.
   *
   * CommonJS variables like `module` and `exports` are treated as global variables in ES modules and may not work as expected.
   *
   * {@include ../docs/checks-commonjs-variable-in-esm.md}
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  commonJsVariableInEsm?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when an imported variable is not exported.
   *
   * If the code is importing a variable that is not exported by the imported module, the value will always be `undefined`. This might be a mistake in the code.
   *
   * {@include ../docs/checks-import-is-undefined.md}
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  importIsUndefined?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when `import.meta` is not supported with the output format and is replaced with an empty object (`{}`).
   *
   * See [`import.meta` in Non-ESM Output Formats page](https://rolldown.rs/in-depth/non-esm-output-formats#import-meta) for more details.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  emptyImportMeta?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when detecting tolerated transform.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  toleratedTransform?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when a namespace is called as a function.
   *
   * A module namespace object is an object and not a function. Calling it as a function will cause a runtime error.
   *
   * {@include ../docs/checks-cannot-call-namespace.md}
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  cannotCallNamespace?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when a config value is overridden by another config value with a higher priority.
   *
   * {@include ../docs/checks-configuration-field-conflict.md}
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  configurationFieldConflict?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when a plugin that is covered by a built-in feature is used.
   *
   * Using built-in features is generally more performant than using plugins.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  preferBuiltinFeature?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when Rolldown could not clean the output directory.
   *
   * See [`output.cleanDir`](https://rolldown.rs/reference/OutputOptions.cleanDir).
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  couldNotCleanDirectory?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when plugins take significant time during the build process.
   *
   * {@include ../docs/checks-plugin-timings.md}
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  pluginTimings?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when both the code and postBanner contain shebang
   *
   * Having multiple shebangs in a file is a syntax error.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  duplicateShebang?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when a tsconfig option or combination of options is not supported.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  unsupportedTsconfigOption?: false | 'warn' | 'error';

  /**
   * Whether to emit warnings when a module is dynamically imported but also statically imported, making the dynamic import ineffective for code splitting.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  ineffectiveDynamicImport?: false | 'warn' | 'error';

  /**
   * Whether to emit info logs when a barrel module has a very large number of re-exports (more than 5000).
   *
   * Such modules can significantly slow down module resolution. Consider using
   * [`@rolldown/plugin-transform-imports`](https://github.com/rolldown/plugins/tree/main/packages/transform-imports)
   * to rewrite barrel imports at the source level so the barrel file is never loaded.
   *
   * - `false` disables the check.
   * - `'warn'` emits a warning (default when the check is enabled).
   * - `'error'` promotes the emission to a hard build error.
   * @default 'warn'
   * */
  largeBarrelModules?: false | 'warn' | 'error';
}
