import type { ModuleOptions } from '..';

/** @category Plugin APIs */
export interface ModuleInfo extends ModuleOptions {
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
   * All exported variables
   */
  exports: string[];
  /**
   * Whether this module is a user- or plugin-defined entry point.
   */
  isEntry: boolean;
  /**
   * The detected format of the module, based on both its syntax and module definition
   * metadata (such as `package.json` `type` and file extensions like
   * `.mjs`/`.cjs`/`.mts`/`.cts`).
   * - "esm" for ES modules (has `import`/`export` statements or is defined as ESM by
   *   module metadata)
   * - "cjs" for CommonJS modules (uses `module.exports`, `exports`, top-level `return`,
   *   or is defined as CommonJS by module metadata)
   * - "unknown" when the format could not be determined from either syntax or module
   *   definition metadata
   */
  inputFormat: 'esm' | 'cjs' | 'unknown';
}
