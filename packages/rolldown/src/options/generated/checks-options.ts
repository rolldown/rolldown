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
   * Whether to emit warning when detecting mixed exports
   * @default true
   */
  mixedExports?: boolean;

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
   * Whether to emit warning when detecting cannot call namespace
   * @default true
   */
  cannotCallNamespace?: boolean;

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

  /**
   * Whether to emit warning when detecting could not clean directory
   * @default true
   */
  couldNotCleanDirectory?: boolean;

  /**
   * Whether to emit warning when detecting plugin timings
   *
   * When enabled, Rolldown measures time spent in each plugin hook. If plugins significantly impact build performance, a warning is emitted with a breakdown of plugin timings.
   *
   * **How it works:**
   * 1. **Detection threshold**: A warning is triggered when plugin time (total build
   * time minus link stage time) exceeds 100x the link stage time. This threshold was
   * determined by studying plugin impact on real-world projects.
   * 2. **Identifying plugins**: When the threshold is exceeded, Rolldown reports up
   * to 5 plugins that take longer than the average plugin time, sorted by duration.
   * Each plugin shows its percentage of total plugin time.
   * > [!WARNING]
   * > For hooks using `ctx.resolve()` or `ctx.load()`, the reported time includes
   * waiting for other plugins, which may overestimate that plugin's actual cost.
   * >
   * > Additionally, since plugin hooks execute concurrently, the statistics
   * represent accumulated time rather than wall-clock time. The measured duration
   * also includes Rust-side processing overhead, Tokio async scheduling overhead,
   * NAPI data conversion overhead, and JavaScript event loop overhead.
   *
   * @default true
   */
  pluginTimings?: boolean;
}
