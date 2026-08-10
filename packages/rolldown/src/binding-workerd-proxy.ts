// Workerd bundles alias `src/binding.cjs` to this module, so the pipeline runs
// against a managed per-instance threadless WASI binding instead of a
// process-global native addon. Module evaluation must stay side-effect free:
// every export defers to the currently entered instance.
//
// The export list mirrors the `napi-rs-artifact-metadata` header of
// `src/rolldown-binding.wasip1.cjs` minus the seven private CurrentThread host
// exports (the deferred loader registers and hides those itself);
// `tests/workerd-build.test.ts` keeps the two lists in sync.
import type * as binding from './binding.cjs';
import type { BindingRuntimeCapabilities } from './binding.cjs';

type BindingExports = Record<PropertyKey, unknown>;

let activeExports: BindingExports | undefined;
let activeRefs = 0;
const cachedEnums = new Map<string, object>();

/** @internal Marker checked by `workerd-build.ts` to detect workerd bundles. */
export const __isWorkerdBindingProxy = true;

/** @internal Route the pipeline's binding calls to this instance's exports. */
export function __enterWorkerdBinding(exports: object): void {
  if (exports === null || (typeof exports !== 'object' && typeof exports !== 'function')) {
    throw new TypeError('A workerd Rolldown instance exports object is required');
  }
  if (activeExports !== undefined && activeExports !== exports) {
    throw new Error(
      'Another workerd Rolldown instance is currently active in this module; ' +
        'finish or close builds on it first',
    );
  }
  activeExports = exports as BindingExports;
  activeRefs += 1;
}

/** @internal Release one `__enterWorkerdBinding` reference. */
export function __exitWorkerdBinding(exports: object): void {
  if (activeExports === undefined || activeExports !== exports) return;
  activeRefs -= 1;
  if (activeRefs <= 0) {
    activeRefs = 0;
    activeExports = undefined;
  }
}

function inactiveBindingError(name: string): Error {
  return new Error(
    `Rolldown workerd binding '${name}' was used outside an active build; ` +
      `call build()/createWorkerdBundle() with a live workerd Rolldown instance`,
  );
}

function req(name: string): any {
  const active = activeExports;
  const value = active === undefined ? undefined : active[name];
  if (value === undefined) {
    throw inactiveBindingError(name);
  }
  return value;
}

function fnExport(name: string): (...args: unknown[]) => any {
  return (...args: unknown[]) => req(name)(...args);
}

function classExport(name: string): any {
  const shadow = class {};
  try {
    Object.defineProperty(shadow, 'name', { configurable: true, value: name });
  } catch {
    // The shadow name is diagnostic only.
  }
  return new Proxy(shadow, {
    construct: (_target, args) => Reflect.construct(req(name), args),
    get: (target, property, receiver) => {
      if (activeExports === undefined && (property === 'name' || property === 'prototype')) {
        // Keep harmless diagnostic reads (and `instanceof` fallbacks, which
        // read `prototype`) from throwing while no instance is active.
        return Reflect.get(target, property, receiver);
      }
      return Reflect.get(req(name), property);
    },
    has: (_target, property) => Reflect.has(req(name), property),
  });
}

function enumExport(name: string): any {
  return new Proxy(Object.create(null) as object, {
    get: (_target, property) => {
      let source = cachedEnums.get(name);
      if (source === undefined) {
        // Enum objects are artifact constants; cache the first successful read.
        source = req(name) as object;
        cachedEnums.set(name, source);
      }
      return Reflect.get(source, property);
    },
    has: (_target, property) => Reflect.has(req(name) as object, property),
    ownKeys: () => Reflect.ownKeys(req(name) as object),
    getOwnPropertyDescriptor: (_target, property) => {
      const descriptor = Reflect.getOwnPropertyDescriptor(req(name) as object, property);
      if (descriptor) descriptor.configurable = true;
      return descriptor;
    },
  });
}

// Matches the generated loader export consumed by
// `runtime-support.ts#getLoadedBindingTarget()`.
export const __rolldownBindingTarget = 'wasi';

// Report used while no managed instance is active, so importing
// `runtime-lifecycle.ts` (which reads capabilities at module evaluation) is
// safe in workerd bundles. Matches `async_runtime.rs get_runtime_capabilities()`
// for wasm32-wasip1 before any timer driver registration (`timers: false`).
const STATIC_THREADLESS_CAPABILITIES: BindingRuntimeCapabilities = Object.freeze({
  asyncRuntimeBuild: true,
  backend: 'shared',
  blockOnJsThreadSafe: false,
  devSupported: false,
  flavor: 'CurrentThread',
  target: 'wasi',
  threads: false,
  timers: false,
  wasi: true,
  watchSupported: false,
});

export function getRuntimeCapabilities(): BindingRuntimeCapabilities {
  const active = activeExports;
  if (active !== undefined) {
    const reporter = active['getRuntimeCapabilities'];
    if (typeof reporter === 'function') {
      return Reflect.apply(reporter, undefined, []) as BindingRuntimeCapabilities;
    }
  }
  return STATIC_THREADLESS_CAPABILITIES;
}

// -- functions ---------------------------------------------------------------
export const acquireAsyncRuntime: typeof binding.acquireAsyncRuntime = fnExport(
  'acquireAsyncRuntime',
) as typeof binding.acquireAsyncRuntime;
export const collapseSourcemaps: typeof binding.collapseSourcemaps = fnExport(
  'collapseSourcemaps',
) as typeof binding.collapseSourcemaps;
export const configureAsyncRuntime: typeof binding.configureAsyncRuntime = fnExport(
  'configureAsyncRuntime',
) as typeof binding.configureAsyncRuntime;
export const enhancedTransform: typeof binding.enhancedTransform = fnExport(
  'enhancedTransform',
) as typeof binding.enhancedTransform;
export const enhancedTransformSync: typeof binding.enhancedTransformSync = fnExport(
  'enhancedTransformSync',
) as typeof binding.enhancedTransformSync;
export const getAsyncRuntimeConfig: typeof binding.getAsyncRuntimeConfig = fnExport(
  'getAsyncRuntimeConfig',
) as typeof binding.getAsyncRuntimeConfig;
export const getAsyncRuntimeMetrics: typeof binding.getAsyncRuntimeMetrics = fnExport(
  'getAsyncRuntimeMetrics',
) as typeof binding.getAsyncRuntimeMetrics;
export const initTraceSubscriber: typeof binding.initTraceSubscriber = fnExport(
  'initTraceSubscriber',
) as typeof binding.initTraceSubscriber;
export const isolatedDeclaration: typeof binding.isolatedDeclaration = fnExport(
  'isolatedDeclaration',
) as typeof binding.isolatedDeclaration;
export const isolatedDeclarationSync: typeof binding.isolatedDeclarationSync = fnExport(
  'isolatedDeclarationSync',
) as typeof binding.isolatedDeclarationSync;
export const minify: typeof binding.minify = fnExport('minify') as typeof binding.minify;
export const minifySync: typeof binding.minifySync = fnExport(
  'minifySync',
) as typeof binding.minifySync;
export const moduleRunnerTransform: typeof binding.moduleRunnerTransform = fnExport(
  'moduleRunnerTransform',
) as typeof binding.moduleRunnerTransform;
export const moduleRunnerTransformSync: typeof binding.moduleRunnerTransformSync = fnExport(
  'moduleRunnerTransformSync',
) as typeof binding.moduleRunnerTransformSync;
export const parse: typeof binding.parse = fnExport('parse') as typeof binding.parse;
export const parseSync: typeof binding.parseSync = fnExport(
  'parseSync',
) as typeof binding.parseSync;
export const rawTransferSupported: typeof binding.rawTransferSupported = fnExport(
  'rawTransferSupported',
) as typeof binding.rawTransferSupported;
export const registerPlugins: typeof binding.registerPlugins = fnExport(
  'registerPlugins',
) as typeof binding.registerPlugins;
export const resetAsyncRuntimeMetrics: typeof binding.resetAsyncRuntimeMetrics = fnExport(
  'resetAsyncRuntimeMetrics',
) as typeof binding.resetAsyncRuntimeMetrics;
export const resolveTsconfig: typeof binding.resolveTsconfig = fnExport(
  'resolveTsconfig',
) as typeof binding.resolveTsconfig;
export const shutdownAsyncRuntime: typeof binding.shutdownAsyncRuntime = fnExport(
  'shutdownAsyncRuntime',
) as typeof binding.shutdownAsyncRuntime;
export const startAsyncRuntime: typeof binding.startAsyncRuntime = fnExport(
  'startAsyncRuntime',
) as typeof binding.startAsyncRuntime;
export const sync: typeof binding.sync = fnExport('sync') as typeof binding.sync;
export const transform: typeof binding.transform = fnExport(
  'transform',
) as typeof binding.transform;
export const transformSync: typeof binding.transformSync = fnExport(
  'transformSync',
) as typeof binding.transformSync;

// -- classes -----------------------------------------------------------------
export const BindingAsyncRuntimeLease: typeof binding.BindingAsyncRuntimeLease = classExport(
  'BindingAsyncRuntimeLease',
) as typeof binding.BindingAsyncRuntimeLease;
export const BindingBundleEndEventData: typeof binding.BindingBundleEndEventData = classExport(
  'BindingBundleEndEventData',
) as typeof binding.BindingBundleEndEventData;
export const BindingBundleErrorEventData: typeof binding.BindingBundleErrorEventData = classExport(
  'BindingBundleErrorEventData',
) as typeof binding.BindingBundleErrorEventData;
export const BindingBundler: typeof binding.BindingBundler = classExport(
  'BindingBundler',
) as typeof binding.BindingBundler;
export const BindingBundleStartEventData: typeof binding.BindingBundleStartEventData = classExport(
  'BindingBundleStartEventData',
) as typeof binding.BindingBundleStartEventData;
export const BindingCallableBuiltinPlugin: typeof binding.BindingCallableBuiltinPlugin =
  classExport('BindingCallableBuiltinPlugin') as typeof binding.BindingCallableBuiltinPlugin;
export const BindingChunkingContext: typeof binding.BindingChunkingContext = classExport(
  'BindingChunkingContext',
) as typeof binding.BindingChunkingContext;
export const BindingDecodedMap: typeof binding.BindingDecodedMap = classExport(
  'BindingDecodedMap',
) as typeof binding.BindingDecodedMap;
export const BindingDevEngine: typeof binding.BindingDevEngine = classExport(
  'BindingDevEngine',
) as typeof binding.BindingDevEngine;
export const BindingLoadPluginContext: typeof binding.BindingLoadPluginContext = classExport(
  'BindingLoadPluginContext',
) as typeof binding.BindingLoadPluginContext;
export const BindingMagicString: typeof binding.BindingMagicString = classExport(
  'BindingMagicString',
) as typeof binding.BindingMagicString;
export const BindingModuleInfo: typeof binding.BindingModuleInfo = classExport(
  'BindingModuleInfo',
) as typeof binding.BindingModuleInfo;
export const BindingNormalizedOptions: typeof binding.BindingNormalizedOptions = classExport(
  'BindingNormalizedOptions',
) as typeof binding.BindingNormalizedOptions;
export const BindingOutputAsset: typeof binding.BindingOutputAsset = classExport(
  'BindingOutputAsset',
) as typeof binding.BindingOutputAsset;
export const BindingOutputChunk: typeof binding.BindingOutputChunk = classExport(
  'BindingOutputChunk',
) as typeof binding.BindingOutputChunk;
export const BindingPluginContext: typeof binding.BindingPluginContext = classExport(
  'BindingPluginContext',
) as typeof binding.BindingPluginContext;
export const BindingRenderedChunk: typeof binding.BindingRenderedChunk = classExport(
  'BindingRenderedChunk',
) as typeof binding.BindingRenderedChunk;
export const BindingRenderedChunkMeta: typeof binding.BindingRenderedChunkMeta = classExport(
  'BindingRenderedChunkMeta',
) as typeof binding.BindingRenderedChunkMeta;
export const BindingRenderedModule: typeof binding.BindingRenderedModule = classExport(
  'BindingRenderedModule',
) as typeof binding.BindingRenderedModule;
export const BindingSourceMap: typeof binding.BindingSourceMap = classExport(
  'BindingSourceMap',
) as typeof binding.BindingSourceMap;
export const BindingTransformPluginContext: typeof binding.BindingTransformPluginContext =
  classExport('BindingTransformPluginContext') as typeof binding.BindingTransformPluginContext;
export const BindingWatcher: typeof binding.BindingWatcher = classExport(
  'BindingWatcher',
) as typeof binding.BindingWatcher;
export const BindingWatcherBundler: typeof binding.BindingWatcherBundler = classExport(
  'BindingWatcherBundler',
) as typeof binding.BindingWatcherBundler;
export const BindingWatcherChangeData: typeof binding.BindingWatcherChangeData = classExport(
  'BindingWatcherChangeData',
) as typeof binding.BindingWatcherChangeData;
export const BindingWatcherEvent: typeof binding.BindingWatcherEvent = classExport(
  'BindingWatcherEvent',
) as typeof binding.BindingWatcherEvent;
export const ParallelJsPluginRegistry: typeof binding.ParallelJsPluginRegistry = classExport(
  'ParallelJsPluginRegistry',
) as typeof binding.ParallelJsPluginRegistry;
export const ParseResult: typeof binding.ParseResult = classExport(
  'ParseResult',
) as typeof binding.ParseResult;
export const ResolverFactory: typeof binding.ResolverFactory = classExport(
  'ResolverFactory',
) as typeof binding.ResolverFactory;
export const TraceSubscriberGuard: typeof binding.TraceSubscriberGuard = classExport(
  'TraceSubscriberGuard',
) as typeof binding.TraceSubscriberGuard;
export const TsconfigCache: typeof binding.TsconfigCache = classExport(
  'TsconfigCache',
) as typeof binding.TsconfigCache;

// -- enums -------------------------------------------------------------------
export const BindingAttachDebugInfo: typeof binding.BindingAttachDebugInfo = enumExport(
  'BindingAttachDebugInfo',
) as typeof binding.BindingAttachDebugInfo;
export const BindingChunkModuleOrderBy: typeof binding.BindingChunkModuleOrderBy = enumExport(
  'BindingChunkModuleOrderBy',
) as typeof binding.BindingChunkModuleOrderBy;
export const BindingLogLevel: typeof binding.BindingLogLevel = enumExport(
  'BindingLogLevel',
) as typeof binding.BindingLogLevel;
export const BindingPluginOrder: typeof binding.BindingPluginOrder = enumExport(
  'BindingPluginOrder',
) as typeof binding.BindingPluginOrder;
export const BindingPropertyReadSideEffects: typeof binding.BindingPropertyReadSideEffects =
  enumExport('BindingPropertyReadSideEffects') as typeof binding.BindingPropertyReadSideEffects;
export const BindingPropertyWriteSideEffects: typeof binding.BindingPropertyWriteSideEffects =
  enumExport('BindingPropertyWriteSideEffects') as typeof binding.BindingPropertyWriteSideEffects;
export const BindingRebuildStrategy: typeof binding.BindingRebuildStrategy = enumExport(
  'BindingRebuildStrategy',
) as typeof binding.BindingRebuildStrategy;
export const EnforceExtension: typeof binding.EnforceExtension = enumExport(
  'EnforceExtension',
) as typeof binding.EnforceExtension;

// -- constant string-union objects (runtime values of dts `export type`s) ----
export const BindingBuiltinPluginName: Record<string, unknown> = enumExport(
  'BindingBuiltinPluginName',
) as Record<string, unknown>;
export const BindingErrorStage: Record<string, unknown> = enumExport('BindingErrorStage') as Record<
  string,
  unknown
>;
export const BindingRuntimeFlavor: Record<string, unknown> = enumExport(
  'BindingRuntimeFlavor',
) as Record<string, unknown>;
export const ExportExportNameKind: Record<string, unknown> = enumExport(
  'ExportExportNameKind',
) as Record<string, unknown>;
export const ExportImportNameKind: Record<string, unknown> = enumExport(
  'ExportImportNameKind',
) as Record<string, unknown>;
export const ExportLocalNameKind: Record<string, unknown> = enumExport(
  'ExportLocalNameKind',
) as Record<string, unknown>;
export const FilterTokenKind: Record<string, unknown> = enumExport('FilterTokenKind') as Record<
  string,
  unknown
>;
export const HelperMode: Record<string, unknown> = enumExport('HelperMode') as Record<
  string,
  unknown
>;
export const ImportNameKind: Record<string, unknown> = enumExport('ImportNameKind') as Record<
  string,
  unknown
>;
export const LegalCommentsMode: Record<string, unknown> = enumExport('LegalCommentsMode') as Record<
  string,
  unknown
>;
export const ModuleType: Record<string, unknown> = enumExport('ModuleType') as Record<
  string,
  unknown
>;
export const Severity: Record<string, unknown> = enumExport('Severity') as Record<string, unknown>;
