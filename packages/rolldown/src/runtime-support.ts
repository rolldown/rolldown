import * as binding from './binding.cjs';
import type { BindingRuntimeCapabilities } from './binding.cjs';
import { BindingMismatchError } from './utils/binding-mismatch-error';

// Workflow feature matrix derived from the binding's self-reported runtime
// capabilities; JS gates dev/watch/parallel-plugin surfaces on these flags
// instead of sniffing the artifact target.
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
type BindingRuntimeTarget = BindingRuntimeCapabilities['target'];

const LOADED_BINDING_TARGET_EXPORT = '__rolldownBindingTarget';
const RUNTIME_BACKENDS = ['tokio', 'shared'] as const;
const RUNTIME_FLAVORS = ['CurrentThread', 'MultiThread'] as const;
const RUNTIME_TARGETS = ['native', 'wasi', 'wasi-threads'] as const;

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

  constructor(
    feature: RuntimeFeature,
    runtime: BindingRuntimeCapabilities = getRuntimeCapabilitiesCompat(),
  ) {
    runtime = normalizeRuntimeCapabilities(runtime);
    const verb = feature === 'parallelPlugins' ? 'are' : 'is';
    const runtimeDescription = `Rolldown's ${runtime.flavor} runtime on the ${runtime.target} target`;
    const message = getRuntimeSupport(runtime)[feature]
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
 * This intentionally sits above low-level scheduler capabilities so stacked
 * runtime integrations can extend it with host-specific feature support.
 */
export function getRuntimeSupport(
  runtime: BindingRuntimeCapabilities = getRuntimeCapabilitiesCompat(),
): RuntimeSupport {
  runtime = normalizeRuntimeCapabilities(runtime);
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
  const runtime = getRuntimeCapabilitiesCompat();
  if (!getRuntimeSupport(runtime)[feature]) {
    throw new UnsupportedRuntimeFeatureError(feature, runtime);
  }
}

/**
 * Report the loaded binding's normalized runtime capabilities.
 *
 * Older binding reports are completed where the missing fields have stable
 * compatibility defaults. Malformed or mismatched reports fail closed.
 */
export function getRuntimeCapabilitiesCompat(): BindingRuntimeCapabilities {
  return getRuntimeCapabilityReportCompat().capabilities;
}

export function getRuntimeCapabilityReportCompat(): {
  capabilities: BindingRuntimeCapabilities;
  hasReporter: boolean;
} {
  const getRuntimeCapabilities = readBindingExport('getRuntimeCapabilities');
  if (getRuntimeCapabilities === undefined) {
    return {
      capabilities: getLegacyRuntimeCapabilities(),
      hasReporter: false,
    };
  }
  if (typeof getRuntimeCapabilities !== 'function') {
    throw new BindingRuntimeContractError(
      'getRuntimeCapabilities must be a function when the export is present',
    );
  }
  let runtime: unknown;
  try {
    runtime = Reflect.apply(getRuntimeCapabilities, undefined, []);
  } catch (error) {
    throw new BindingRuntimeContractError('getRuntimeCapabilities() threw while reporting', {
      cause: error,
    });
  }
  return {
    capabilities: normalizeRuntimeCapabilities(runtime, getLoadedBindingTarget()),
    hasReporter: true,
  };
}

function getLegacyRuntimeCapabilities(): BindingRuntimeCapabilities {
  const target = getLoadedBindingTarget();
  if (!target) {
    return LEGACY_NATIVE_CAPABILITIES;
  }
  switch (target) {
    case 'wasi':
      return LEGACY_WASI_CAPABILITIES;
    case 'wasi-threads':
      return LEGACY_WASI_THREADS_CAPABILITIES;
    case 'native':
      return LEGACY_NATIVE_CAPABILITIES;
  }
}

function normalizeRuntimeCapabilities(
  runtime: unknown,
  loadedTarget?: BindingRuntimeTarget,
): BindingRuntimeCapabilities {
  if (runtime === null || typeof runtime !== 'object') {
    throw new BindingRuntimeContractError('getRuntimeCapabilities() did not return an object');
  }

  const report = runtime as Record<PropertyKey, unknown>;
  const asyncRuntimeBuild = readBooleanCapability(report, 'asyncRuntimeBuild');
  const backend = readEnumCapability(report, 'backend', RUNTIME_BACKENDS);
  const blockOnJsThreadSafe = readBooleanCapability(report, 'blockOnJsThreadSafe');
  const flavor = readEnumCapability(report, 'flavor', RUNTIME_FLAVORS);
  const target = readEnumCapability(report, 'target', RUNTIME_TARGETS);
  const threads = readBooleanCapability(report, 'threads');
  const timers = readBooleanCapability(report, 'timers');
  const wasi = readBooleanCapability(report, 'wasi');
  const devSupported = readOptionalBooleanCapability(report, 'devSupported') ?? threads;
  const watchSupported = readOptionalBooleanCapability(report, 'watchSupported') ?? !wasi;

  if (asyncRuntimeBuild !== (backend === 'shared')) {
    throw new BindingRuntimeContractError(
      'asyncRuntimeBuild does not agree with the reported backend',
    );
  }
  if (threads !== (flavor === 'MultiThread')) {
    throw new BindingRuntimeContractError('threads does not agree with the reported flavor');
  }
  if (wasi !== (target !== 'native')) {
    throw new BindingRuntimeContractError('wasi does not agree with the reported target');
  }
  // No cross-checks for devSupported/watchSupported: missing fields already
  // took the stable `threads` / inverse-`wasi` compatibility defaults above,
  // but explicit values are independent workflow capabilities and are
  // preserved.
  if (loadedTarget && loadedTarget !== target) {
    throw new BindingRuntimeContractError(
      'getRuntimeCapabilities().target does not match the generated loader target',
    );
  }

  return {
    asyncRuntimeBuild,
    backend,
    blockOnJsThreadSafe,
    devSupported,
    flavor,
    target,
    threads,
    timers,
    wasi,
    watchSupported,
  };
}

function getLoadedBindingTarget(): BindingRuntimeTarget | undefined {
  const target = readBindingExport(LOADED_BINDING_TARGET_EXPORT);
  if (target === undefined) return;
  if (RUNTIME_TARGETS.some((candidate) => candidate === target)) {
    return target as BindingRuntimeTarget;
  }
  throw new BindingRuntimeContractError(
    `the generated loader export ${LOADED_BINDING_TARGET_EXPORT} is invalid`,
  );
}

function readBindingExport(key: string): unknown {
  try {
    if (!Reflect.has(binding, key)) return;
    return Reflect.get(binding, key);
  } catch (error) {
    throw new BindingRuntimeContractError(`the binding export ${key} could not be read`, {
      cause: error,
    });
  }
}

function readCapabilityField(
  report: Record<PropertyKey, unknown>,
  key: keyof BindingRuntimeCapabilities,
): unknown {
  try {
    return Reflect.get(report, key, report);
  } catch (error) {
    throw new BindingRuntimeContractError(`${key} could not be read`, { cause: error });
  }
}

function readBooleanCapability(
  report: Record<PropertyKey, unknown>,
  key: keyof BindingRuntimeCapabilities,
): boolean {
  const value = readCapabilityField(report, key);
  if (typeof value !== 'boolean') {
    throw new BindingRuntimeContractError(`${key} must be a boolean`);
  }
  return value;
}

function readOptionalBooleanCapability(
  report: Record<PropertyKey, unknown>,
  key: 'devSupported' | 'watchSupported',
): boolean | undefined {
  const value = readCapabilityField(report, key);
  if (value === undefined) return;
  if (typeof value !== 'boolean') {
    throw new BindingRuntimeContractError(`${key} must be a boolean when present`);
  }
  return value;
}

function readEnumCapability<const T extends readonly string[]>(
  report: Record<PropertyKey, unknown>,
  key: keyof BindingRuntimeCapabilities,
  values: T,
): T[number] {
  const value = readCapabilityField(report, key);
  if (values.some((candidate) => candidate === value)) {
    return value as T[number];
  }
  throw new BindingRuntimeContractError(`${key} is not a recognized value`);
}

class BindingRuntimeContractError extends BindingMismatchError {
  constructor(detail: string, options?: ErrorOptions) {
    super(
      `The loaded Rolldown binding returned an incompatible runtime capability contract: ` +
        `${detail}. Reinstall Rolldown so the JavaScript package and binding versions match.`,
      options,
    );
    this.name = 'BindingRuntimeContractError';
  }
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

const LEGACY_WASI_CAPABILITIES: BindingRuntimeCapabilities = Object.freeze({
  asyncRuntimeBuild: false,
  backend: 'tokio',
  blockOnJsThreadSafe: false,
  devSupported: false,
  flavor: 'CurrentThread',
  target: 'wasi' satisfies BindingRuntimeTarget,
  threads: false,
  timers: false,
  wasi: true,
  watchSupported: false,
});

const LEGACY_WASI_THREADS_CAPABILITIES: BindingRuntimeCapabilities = Object.freeze({
  asyncRuntimeBuild: false,
  backend: 'tokio',
  blockOnJsThreadSafe: false,
  devSupported: true,
  flavor: 'MultiThread',
  target: 'wasi-threads' satisfies BindingRuntimeTarget,
  threads: true,
  timers: true,
  wasi: true,
  watchSupported: false,
});
