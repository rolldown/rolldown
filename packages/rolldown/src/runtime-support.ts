import * as binding from './binding.cjs';
import type { BindingRuntimeCapabilities } from './binding.cjs';

// See internal-docs/async-runtime/implementation.md.
export interface RuntimeSupport {
  dev: boolean;
  watch: boolean;
  dynamicImportVarsResolver: boolean;
  importGlobResolver: boolean;
  parallelPlugins: boolean;
  pluginErrorMetadata: boolean;
  symlinks: boolean;
  /**
   * Whether the loaded binding is the threadless WASI flavor required by
   * managed workerd loaders. This does not assert that the current package
   * exposes `@rolldown/browser/workerd`.
   */
  threadlessWasi: boolean;
  /**
   * Whether the loaded package exposes its managed workerd entry for this
   * binding. This is true for the threadless `@rolldown/browser` package, not
   * for a standalone threadless binding loaded through another package.
   */
  workerd: boolean;
}

export type RuntimeFeature = keyof RuntimeSupport;

const FEATURE_NAMES: Record<RuntimeFeature, string> = {
  dev: 'dev()',
  watch: 'watch()',
  dynamicImportVarsResolver: 'viteDynamicImportVarsPlugin({ resolver })',
  importGlobResolver: 'viteImportGlobPlugin() package and subpath resolution',
  parallelPlugins: 'parallel JavaScript plugins',
  pluginErrorMetadata: 'structured plugin error metadata',
  symlinks: 'symbolic-link traversal',
  threadlessWasi: 'threadless WASI compatibility',
  workerd: 'the managed workerd loader',
};

const FEATURE_ALTERNATIVES: Record<RuntimeFeature, string> = {
  dev: 'Use a MultiThread runtime.',
  watch: 'Use one-shot builds on WASI or run watch mode with a native binding.',
  dynamicImportVarsResolver: 'Use a native binding.',
  importGlobResolver: 'Use a native binding.',
  parallelPlugins: 'Use a native binding.',
  pluginErrorMetadata: 'Load a supported Rolldown artifact.',
  symlinks: 'Use a native binding.',
  threadlessWasi: 'Use the threadless WASI artifact.',
  workerd: 'Use @rolldown/browser/workerd with the threadless WASI artifact.',
};

export class UnsupportedRuntimeFeatureError extends Error {
  readonly code = 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE';
  readonly feature: RuntimeFeature;
  readonly runtime: BindingRuntimeCapabilities;

  constructor(feature: RuntimeFeature) {
    const runtime = getRuntimeCapabilitiesCompat();
    const verb = feature === 'parallelPlugins' ? 'are' : 'is';
    const runtimeDescription = `Rolldown's ${runtime.flavor} runtime on the ${runtime.target} target`;
    const message = getRuntimeSupport()[feature]
      ? `${FEATURE_NAMES[feature]} ${verb} supported by ${runtimeDescription}. ` +
        `UnsupportedRuntimeFeatureError was constructed for an available feature.`
      : `${FEATURE_NAMES[feature]} ${verb} not supported by ${runtimeDescription}. ` +
        FEATURE_ALTERNATIVES[feature];
    super(message);
    this.name = 'UnsupportedRuntimeFeatureError';
    this.feature = feature;
    this.runtime = runtime;
  }
}

/**
 * Report the stable, user-facing workflow support of the loaded artifact.
 *
 * This intentionally sits above low-level scheduler capabilities.
 */
export function getRuntimeSupport(): RuntimeSupport {
  const runtime = getRuntimeCapabilitiesCompat();
  const threadlessWasi = runtime.target === 'wasi' && !runtime.threads;
  return {
    dev: runtime.devSupported,
    watch: runtime.watchSupported,
    dynamicImportVarsResolver: true,
    importGlobResolver: true,
    parallelPlugins: !runtime.wasi,
    pluginErrorMetadata: true,
    symlinks: !runtime.wasi,
    threadlessWasi,
    workerd: threadlessWasi && import.meta.workerdPackageApi === true,
  };
}

export function assertRuntimeFeature(feature: RuntimeFeature): void {
  if (!getRuntimeSupport()[feature]) {
    throw new UnsupportedRuntimeFeatureError(feature);
  }
}

/** Report the loaded binding's runtime capabilities. */
export function getRuntimeCapabilitiesCompat(): BindingRuntimeCapabilities {
  return binding.getRuntimeCapabilities();
}
