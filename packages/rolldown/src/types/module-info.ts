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
}
