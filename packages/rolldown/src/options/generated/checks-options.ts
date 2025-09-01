// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/checks.rs`

export interface ChecksOptions {
  /**
   * Whether to emit warning when detecting circular dependency
   * @default false
   */
  circularDependency?: boolean;

  /**
   * Whether to emit warning when detecting eval
   * @default true
   */
  eval?: boolean;

  /**
   * Whether to emit warning when detecting missing global name
   * @default true
   */
  missingGlobalName?: boolean;

  /**
   * Whether to emit warning when detecting missing name option for iife export
   * @default true
   */
  missingNameOptionForIifeExport?: boolean;

  /**
   * Whether to emit warning when detecting mixed export
   * @default true
   */
  mixedExport?: boolean;

  /**
   * Whether to emit warning when detecting unresolved entry
   * @default true
   */
  unresolvedEntry?: boolean;

  /**
   * Whether to emit warning when detecting unresolved import
   * @default true
   */
  unresolvedImport?: boolean;

  /**
   * Whether to emit warning when detecting filename conflict
   * @default true
   */
  filenameConflict?: boolean;

  /**
   * Whether to emit warning when detecting common js variable in esm
   * @default true
   */
  commonJsVariableInEsm?: boolean;

  /**
   * Whether to emit warning when detecting import is undefined
   * @default true
   */
  importIsUndefined?: boolean;

  /**
   * Whether to emit warning when detecting empty import meta
   * @default true
   */
  emptyImportMeta?: boolean;

  /**
   * Whether to emit warning when detecting configuration field conflict
   * @default true
   */
  configurationFieldConflict?: boolean;

  /**
   * Whether to emit warning when detecting prefer builtin feature
   * @default true
   */
  preferBuiltinFeature?: boolean;
}
