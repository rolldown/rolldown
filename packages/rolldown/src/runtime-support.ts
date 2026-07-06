import * as binding from './binding.cjs';
import type { BindingRuntimeCapabilities } from './binding.cjs';

// See internal-docs/async-runtime/implementation.md.
export interface RuntimeSupport {
  dev: boolean;
  watch: boolean;
  parallelPlugins: boolean;
  viteDynamicImportVarsResolver: boolean;
}

export type RuntimeFeature = keyof RuntimeSupport;

const FEATURE_NAMES: Record<RuntimeFeature, string> = {
  dev: 'dev()',
  watch: 'watch()',
  parallelPlugins: 'Parallel JavaScript plugins',
  viteDynamicImportVarsResolver: "viteDynamicImportVarsPlugin()'s resolver option",
};

const FEATURE_ALTERNATIVES: Record<RuntimeFeature, string> = {
  dev: 'Use a MultiThread runtime.',
  watch: 'Use one-shot builds on WASI or run watch mode with a native binding.',
  parallelPlugins: 'Use a native binding.',
  viteDynamicImportVarsResolver: 'Use a MultiThread runtime or omit the resolver option.',
};

export class UnsupportedRuntimeFeatureError extends Error {
  readonly code = 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE';
  readonly feature: RuntimeFeature;
  readonly runtime: BindingRuntimeCapabilities;

  constructor(
    feature: RuntimeFeature,
    runtime: BindingRuntimeCapabilities = getRuntimeCapabilitiesCompat(),
  ) {
    runtime = normalizeRuntimeCapabilities(runtime);
    const verb = feature === 'parallelPlugins' ? 'are' : 'is';
    super(
      `${FEATURE_NAMES[feature]} ${verb} not supported by Rolldown's ${runtime.flavor} runtime ` +
        `on the ${runtime.target} target. ${FEATURE_ALTERNATIVES[feature]}`,
    );
    this.name = 'UnsupportedRuntimeFeatureError';
    this.feature = feature;
    this.runtime = runtime;
  }
}

/**
 * Report the stable, user-facing workflow support of the loaded artifact.
 *
 * This intentionally sits above low-level scheduler capabilities so stacked
 * runtime integrations can extend it with host-specific feature support.
 */
export function getRuntimeSupport(
  runtime: BindingRuntimeCapabilities = getRuntimeCapabilitiesCompat(),
): RuntimeSupport {
  runtime = normalizeRuntimeCapabilities(runtime);
  return {
    dev: runtime.devSupported,
    watch: runtime.watchSupported,
    parallelPlugins: !runtime.wasi,
    viteDynamicImportVarsResolver: runtime.threads,
  };
}

export function assertRuntimeFeature(feature: RuntimeFeature): void {
  const runtime = getRuntimeCapabilitiesCompat();
  if (!getRuntimeSupport(runtime)[feature]) {
    throw new UnsupportedRuntimeFeatureError(feature, runtime);
  }
}

function getRuntimeCapabilitiesCompat(): BindingRuntimeCapabilities {
  const getRuntimeCapabilities = (binding as Record<PropertyKey, unknown>).getRuntimeCapabilities;
  if (typeof getRuntimeCapabilities !== 'function') {
    return LEGACY_NATIVE_CAPABILITIES;
  }
  return normalizeRuntimeCapabilities(
    (getRuntimeCapabilities as (this: void) => BindingRuntimeCapabilities)(),
  );
}

function normalizeRuntimeCapabilities(
  runtime: BindingRuntimeCapabilities,
): BindingRuntimeCapabilities {
  const legacy = runtime as BindingRuntimeCapabilities & {
    devSupported?: boolean;
    watchSupported?: boolean;
  };
  if (typeof legacy.devSupported === 'boolean' && typeof legacy.watchSupported === 'boolean') {
    return runtime;
  }
  return {
    ...runtime,
    devSupported: typeof legacy.devSupported === 'boolean' ? legacy.devSupported : runtime.threads,
    watchSupported:
      typeof legacy.watchSupported === 'boolean' ? legacy.watchSupported : !runtime.wasi,
  };
}

const LEGACY_NATIVE_CAPABILITIES: BindingRuntimeCapabilities = Object.freeze({
  asyncRuntimeBuild: false,
  backend: 'tokio',
  blockOnJsThreadSafe: false,
  devSupported: true,
  flavor: 'MultiThread',
  target: 'native',
  threads: true,
  timers: true,
  wasi: false,
  watchSupported: true,
});
