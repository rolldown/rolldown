import {
  emnapiAsyncWorkPlugin as __emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin as __emnapiTSFNPlugin,
  createOnMessage as __wasmCreateOnMessageForFsProxy,
  instantiateNapiModule as __emnapiInstantiateNapiModule,
  WASI as __WASI,
} from '@napi-rs/wasm-runtime'
import { createContext as __emnapiCreateContext } from '@emnapi/runtime'
import { memfs, Buffer } from '@napi-rs/wasm-runtime/fs'


export const { fs: __fs, vol: __volume } = memfs()

const __wasi = new __WASI({
  version: 'preview1',
  fs: __fs,
  preopens: {
    '/': '/',
  },
})

const __wasmUrl = new URL('./rolldown-binding.wasm32-wasi.wasm', import.meta.url).href
const __wasmResponse = await globalThis.fetch(__wasmUrl)
if (!__wasmResponse.ok) {
  throw new Error(
    'Failed to fetch WASI module ' + __wasmUrl + ': ' +
      __wasmResponse.status + ' ' +
      (__wasmResponse.statusText || 'Unknown Status'),
  )
}
const __wasmFile = await __wasmResponse.arrayBuffer()

const __sharedMemory = new WebAssembly.Memory({
  initial: 16384,
  maximum: 65536,
  shared: true,
})

const __emnapiContext = __emnapiCreateContext()

const __wasiInitializationWorkers = new Set()

async function __terminateWasiInitializationWorkers() {
  const __terminations = []
  for (const __worker of __wasiInitializationWorkers) {
    __wasiInitializationWorkers.delete(__worker)
    try {
      __terminations.push(
        Promise.resolve(__worker.terminate()).then(
          () => undefined,
          (__cleanupError) => ({ error: __cleanupError }),
        ),
      )
    } catch (__cleanupError) {
      __terminations.push(Promise.resolve({ error: __cleanupError }))
    }
  }
  const __terminationResults = await Promise.all(__terminations)
  const __cleanupErrors = []
  for (const __terminationResult of __terminationResults) {
    if (__terminationResult !== undefined) {
      __cleanupErrors.push(__terminationResult.error)
    }
  }
  return __cleanupErrors
}


function __createInitializationCleanupError(__error, __cleanupErrors) {
  let __message = 'WASI module initialization failed'
  try {
    if (__error && typeof __error.message === 'string') {
      __message = __error.message
    }
  } catch {}
  const __errors = [__error, ...__cleanupErrors]
  const __AggregateError = globalThis.AggregateError
  const __combinedError =
    typeof __AggregateError === 'function'
      ? new __AggregateError(__errors, __message)
      : new Error(__message)
  if (!('errors' in __combinedError)) {
    __combinedError.errors = __errors
  }
  __combinedError.cause = __error
  return __combinedError
}

let __napiInstance
let __wasiModule
let __napiModule

let __emnapiWasmEnvCleanupPrepared = false

if (__emnapiContext !== undefined) {
  // A raw destroy call on the emnapi context (bypassing
  // __destroyEmnapiContext) must still settle pending napi async work: run
  // the wasm-side cleanup preparation while the environment can still call
  // into JavaScript, then delegate to the original destroy.
  // oxlint-disable-next-line typescript/unbound-method -- invoked with the wrapper receiver below
  const __emnapiContextDestroy = __emnapiContext.destroy
  __emnapiContext.destroy = function() {
    if (!__emnapiWasmEnvCleanupPrepared) {
      const __prepareWasmEnvCleanup =
        __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
      if (typeof __prepareWasmEnvCleanup === 'function') {
        __prepareWasmEnvCleanup()
      }
      __emnapiWasmEnvCleanupPrepared = true
    }
    return Reflect.apply(__emnapiContextDestroy, this, arguments)
  }
}

function __destroyEmnapiContext() {
  if (!__emnapiWasmEnvCleanupPrepared) {
    const __prepareWasmEnvCleanup =
      __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
    if (typeof __prepareWasmEnvCleanup === 'function') {
      __prepareWasmEnvCleanup()
    }
    __emnapiWasmEnvCleanupPrepared = true
  }
  return __emnapiContext.destroy()
}

try {
  __emnapiContext.features.Buffer = Buffer

  ;({
    instance: __napiInstance,
    module: __wasiModule,
    napiModule: __napiModule,
  } = await __emnapiInstantiateNapiModule(__wasmFile, {
    context: __emnapiContext,
    asyncWorkPoolSize: 4,
    plugins: [__emnapiAsyncWorkPlugin, __emnapiTSFNPlugin],
    wasi: __wasi,
    onCreateWorker() {
      const worker = new Worker(new URL('./wasi-worker-browser.mjs', import.meta.url), {
        type: 'module',
      })
      __wasiInitializationWorkers.add(worker)
      worker.addEventListener('message', __wasmCreateOnMessageForFsProxy(__fs))

      worker.addEventListener('message', (event) => {
        if (event.data && typeof event.data === 'object' && event.data.type === 'error') {
          const __CustomEvent = globalThis.CustomEvent
          if (
            typeof globalThis.dispatchEvent === 'function' &&
            typeof __CustomEvent === 'function'
          ) {
            globalThis.dispatchEvent(
              new __CustomEvent('napi-rs-worker-error', { detail: event.data }),
            )
          }
        }
      })

      return worker
    },
    overwriteImports(importObject) {
      importObject.env = {
        ...importObject.env,
        ...importObject.napi,
        ...importObject.emnapi,
        memory: __sharedMemory,
      }
      return importObject
    },
    beforeInit({ instance }) {
      __napiInstance = instance
      for (const name of Object.keys(instance.exports)) {
        if (name.startsWith('__napi_register__')) {
          instance.exports[name]()
        }
      }
    },
  }))
  __wasiInitializationWorkers.clear()
} catch (__error) {
  const __cleanupErrors = []
  try {
    await Promise.resolve(__destroyEmnapiContext())
  } catch (__cleanupError) {
    __cleanupErrors.push(__cleanupError)
  }
  __cleanupErrors.push(...await __terminateWasiInitializationWorkers())
  if (__cleanupErrors.length > 0) {
    throw __createInitializationCleanupError(__error, __cleanupErrors)
  }
  throw __error
}
export default __napiModule.exports
export const __rolldownBindingTarget = 'wasi-threads'
export const LegalCommentsMode = __napiModule.exports.LegalCommentsMode
export const minify = __napiModule.exports.minify
export const minifySync = __napiModule.exports.minifySync
export const Severity = __napiModule.exports.Severity
export const ParseResult = __napiModule.exports.ParseResult
export const ExportExportNameKind = __napiModule.exports.ExportExportNameKind
export const ExportImportNameKind = __napiModule.exports.ExportImportNameKind
export const ExportLocalNameKind = __napiModule.exports.ExportLocalNameKind
export const ImportNameKind = __napiModule.exports.ImportNameKind
export const parse = __napiModule.exports.parse
export const parseSync = __napiModule.exports.parseSync
export const rawTransferSupported = __napiModule.exports.rawTransferSupported
export const ResolverFactory = __napiModule.exports.ResolverFactory
export const EnforceExtension = __napiModule.exports.EnforceExtension
export const ModuleType = __napiModule.exports.ModuleType
export const sync = __napiModule.exports.sync
export const HelperMode = __napiModule.exports.HelperMode
export const isolatedDeclaration = __napiModule.exports.isolatedDeclaration
export const isolatedDeclarationSync = __napiModule.exports.isolatedDeclarationSync
export const moduleRunnerTransform = __napiModule.exports.moduleRunnerTransform
export const moduleRunnerTransformSync = __napiModule.exports.moduleRunnerTransformSync
export const transform = __napiModule.exports.transform
export const transformSync = __napiModule.exports.transformSync
export const BindingAsyncRuntimeLease = __napiModule.exports.BindingAsyncRuntimeLease
export const BindingBundleEndEventData = __napiModule.exports.BindingBundleEndEventData
export const BindingBundleErrorEventData = __napiModule.exports.BindingBundleErrorEventData
export const BindingBundler = __napiModule.exports.BindingBundler
export const BindingBundleStartEventData = __napiModule.exports.BindingBundleStartEventData
export const BindingCallableBuiltinPlugin = __napiModule.exports.BindingCallableBuiltinPlugin
export const BindingChunkingContext = __napiModule.exports.BindingChunkingContext
export const BindingDecodedMap = __napiModule.exports.BindingDecodedMap
export const BindingDevEngine = __napiModule.exports.BindingDevEngine
export const BindingLoadPluginContext = __napiModule.exports.BindingLoadPluginContext
export const BindingMagicString = __napiModule.exports.BindingMagicString
export const BindingModuleInfo = __napiModule.exports.BindingModuleInfo
export const BindingNormalizedOptions = __napiModule.exports.BindingNormalizedOptions
export const BindingOutputAsset = __napiModule.exports.BindingOutputAsset
export const BindingOutputChunk = __napiModule.exports.BindingOutputChunk
export const BindingPluginContext = __napiModule.exports.BindingPluginContext
export const BindingRenderedChunk = __napiModule.exports.BindingRenderedChunk
export const BindingRenderedChunkMeta = __napiModule.exports.BindingRenderedChunkMeta
export const BindingRenderedModule = __napiModule.exports.BindingRenderedModule
export const BindingSourceMap = __napiModule.exports.BindingSourceMap
export const BindingTransformPluginContext = __napiModule.exports.BindingTransformPluginContext
export const BindingWatcher = __napiModule.exports.BindingWatcher
export const BindingWatcherBundler = __napiModule.exports.BindingWatcherBundler
export const BindingWatcherChangeData = __napiModule.exports.BindingWatcherChangeData
export const BindingWatcherEvent = __napiModule.exports.BindingWatcherEvent
export const ParallelJsPluginRegistry = __napiModule.exports.ParallelJsPluginRegistry
export const TraceSubscriberGuard = __napiModule.exports.TraceSubscriberGuard
export const TsconfigCache = __napiModule.exports.TsconfigCache
export const acquireAsyncRuntime = __napiModule.exports.acquireAsyncRuntime
export const BindingAttachDebugInfo = __napiModule.exports.BindingAttachDebugInfo
export const BindingBuiltinPluginName = __napiModule.exports.BindingBuiltinPluginName
export const BindingChunkModuleOrderBy = __napiModule.exports.BindingChunkModuleOrderBy
export const BindingErrorStage = __napiModule.exports.BindingErrorStage
export const BindingLogLevel = __napiModule.exports.BindingLogLevel
export const BindingPluginOrder = __napiModule.exports.BindingPluginOrder
export const BindingPropertyReadSideEffects = __napiModule.exports.BindingPropertyReadSideEffects
export const BindingPropertyWriteSideEffects = __napiModule.exports.BindingPropertyWriteSideEffects
export const BindingRebuildStrategy = __napiModule.exports.BindingRebuildStrategy
export const BindingRuntimeFlavor = __napiModule.exports.BindingRuntimeFlavor
export const collapseSourcemaps = __napiModule.exports.collapseSourcemaps
export const configureAsyncRuntime = __napiModule.exports.configureAsyncRuntime
export const enhancedTransform = __napiModule.exports.enhancedTransform
export const enhancedTransformSync = __napiModule.exports.enhancedTransformSync
export const FilterTokenKind = __napiModule.exports.FilterTokenKind
export const getAsyncRuntimeConfig = __napiModule.exports.getAsyncRuntimeConfig
export const getAsyncRuntimeMetrics = __napiModule.exports.getAsyncRuntimeMetrics
export const getCurrentThreadTaskHostContractVersion = __napiModule.exports.getCurrentThreadTaskHostContractVersion
export const getRuntimeCapabilities = __napiModule.exports.getRuntimeCapabilities
export const initTraceSubscriber = __napiModule.exports.initTraceSubscriber
export const isCurrentThreadHostRegistrationActive = __napiModule.exports.isCurrentThreadHostRegistrationActive
export const registerCurrentThreadTaskHost = __napiModule.exports.registerCurrentThreadTaskHost
export const registerPlugins = __napiModule.exports.registerPlugins
export const registerTimerHost = __napiModule.exports.registerTimerHost
export const reserveCurrentThreadHostRegistration = __napiModule.exports.reserveCurrentThreadHostRegistration
export const resetAsyncRuntimeMetrics = __napiModule.exports.resetAsyncRuntimeMetrics
export const resolveTsconfig = __napiModule.exports.resolveTsconfig
export const shutdownAsyncRuntime = __napiModule.exports.shutdownAsyncRuntime
export const startAsyncRuntime = __napiModule.exports.startAsyncRuntime
export const unregisterCurrentThreadTaskHost = __napiModule.exports.unregisterCurrentThreadTaskHost
export const unregisterTimerHost = __napiModule.exports.unregisterTimerHost
