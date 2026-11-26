import type { ModuleOptions } from '..';

/** @category Plugin APIs */
export interface ModuleInfo extends ModuleOptions {
  /**
   * The `moduleSideEffects` value of the module, which tree shaking starts from. `true` means
   * that tree shaking checks the statements of the module, so a module without side effects can
   * still have `true`.
   *
   * It is the last value that a hook returned or that a plugin wrote to this property: the
   * `resolveId` result that created the module, then the `load` and `transform` results of the
   * module. A hook result of `null` or `undefined` does not change it. When no hook returned a
   * value and no plugin wrote one, it is the first value from these steps:
   *
   * 1. The value that the `treeshake.moduleSideEffects` option gives for the module.
   * 2. For an external module: `false` for a Node.js built-in, and `true` for any other one.
   *    For any other module: the `sideEffects` field of its `package.json`. Rolldown uses this
   *    field only when its own resolver resolved the module. This is also the case when a
   *    `resolveId` hook returned a `this.resolve` result that Rolldown's resolver gave. When a
   *    hook returned its own id, such as a virtual module, Rolldown does not use the field.
   * 3. `true`.
   *
   * Writing `null` or `undefined` clears the value, so the steps above give it again. Earlier
   * hook results and written values do not come back.
   *
   * See [`getModuleInfo`](https://rolldown.rs/reference/Interface.PluginContext#getmoduleinfo)
   * for when the value is available and when a change reaches tree shaking.
   */
  moduleSideEffects: Exclude<ModuleOptions['moduleSideEffects'], undefined>;
  /**
   * @hidden Not supported by Rolldown
   */
  ast: any;
  /**
   * The source code of the module.
   *
   * `null` if external or not yet available.
   */
  code: string | null;
  /**
   * The id of the module for convenience
   */
  id: string;
  /**
   * The ids of all modules that statically import this module.
   */
  importers: string[];
  /**
   * The ids of all modules that dynamically import this module.
   */
  dynamicImporters: string[];
  /**
   * The module ids statically imported by this module.
   */
  importedIds: string[];
  /**
   * The module ids dynamically imported by this module.
   */
  dynamicallyImportedIds: string[];
  /**
   * All exported variables.
   *
   * `*` stands for `export * from`. Names starting with `*` are listed with an additional `*` to keep them apart from it.
   */
  exports: string[];
  /**
   * Whether this module is a user- or plugin-defined entry point.
   */
  isEntry: boolean;
  /**
   * The detected format of the module, based on both its syntax and module definition
   * metadata (such as `package.json` `type` and file extensions like `.mjs`/`.cjs`/`.mts`/`.cts`).
   * - "esm" for ES modules (has `import`/`export` statements or is defined as ESM by module metadata)
   * - "cjs" for CommonJS modules (uses `module.exports`, `exports`, top-level `return`, or is defined as CommonJS by module metadata)
   * - "unknown" when the format could not be determined from either syntax or module definition metadata
   *
   * @experimental
   */
  inputFormat: 'es' | 'cjs' | 'unknown';
}
