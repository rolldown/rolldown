import {
  emnapiAsyncWorkPlugin as __emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin as __emnapiTSFNPlugin,
  createOnMessage as __wasmCreateOnMessageForFsProxy,
  instantiateNapiModule as __emnapiInstantiateNapiModule,
  WASI as __WASI,
} from '@napi-rs/wasm-runtime'
import { createContext as __emnapiCreateContext } from '@emnapi/runtime'
import { memfs } from '@napi-rs/wasm-runtime/fs'


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
    'Failed to fetch WASI module ' +
      __wasmUrl +
      ': ' +
      __wasmResponse.status +
      ' ' +
      (__wasmResponse.statusText || 'Unknown Status'),
  )
}
const __wasmFile = await __wasmResponse.arrayBuffer()

const __sharedMemory = new WebAssembly.Memory({
  initial: 16384,
  maximum: 65536,
  shared: true,
})

let __emnapiContext

const __wasiDisposeSymbol = Symbol.for('napi.rs.wasi.dispose')
const __wasiWorkers = new Set()
let __napiInstance
let __emnapiContextDestroyed = false
let __emnapiContextDestroyPromise
let __emnapiWasmEnvCleanupPrepared = false
let __wasiDisposed = false
let __wasiDisposePromise
let __completeWasiDisposal = function() {}

function __isThenable(value) {
  return (
    value !== null &&
    (typeof value === 'object' || typeof value === 'function') &&
    typeof value.then === 'function'
  )
}

function __createCleanupError(errors, message) {
  if (errors.length === 1) {
    return errors[0]
  }
  const __AggregateError = globalThis.AggregateError
  if (typeof __AggregateError === 'function') {
    return new __AggregateError(errors, message)
  }
  const error = new Error(message)
  error.errors = errors
  return error
}

function __attachCleanupErrors(error, cleanupErrors) {
  if (cleanupErrors.length === 0) {
    return error
  }
  const cleanupError = __createCleanupError(
    cleanupErrors,
    'WASI binding cleanup failed',
  )
  try {
    if (
      error &&
      (typeof error === 'object' || typeof error === 'function')
    ) {
      if (error.cause === undefined) {
        error.cause = cleanupError
        if (error.cause === cleanupError) {
          return error
        }
      }
      if (Array.isArray(error.cleanupErrors)) {
        error.cleanupErrors.push(cleanupError)
        return error
      } else {
        const attachedCleanupErrors = [cleanupError]
        error.cleanupErrors = attachedCleanupErrors
        if (error.cleanupErrors === attachedCleanupErrors) {
          return error
        }
      }
    }
  } catch {}
  const aggregate = __createCleanupError(
    [error, cleanupError],
    'WASI binding initialization and cleanup failed',
  )
  try {
    aggregate.cause = error
  } catch {}
  return aggregate
}

function __prepareWasmEnvCleanup() {
  if (__emnapiWasmEnvCleanupPrepared) {
    return
  }
  const prepare = __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
  if (typeof prepare === 'function') {
    prepare()
  }
  __emnapiWasmEnvCleanupPrepared = true
}

function __destroyEmnapiContext() {
  if (__emnapiContextDestroyed || __emnapiContext === undefined) {
    __emnapiContextDestroyed = true
    return
  }
  if (__emnapiContextDestroyPromise) {
    return __emnapiContextDestroyPromise
  }

  __prepareWasmEnvCleanup()
  const result = __emnapiContext.destroy()
  if (!__isThenable(result)) {
    __emnapiContextDestroyed = true
    return
  }

  const destroyPromise = Promise.resolve(result).then(
    (value) => {
      __emnapiContextDestroyed = true
      return value
    },
    (error) => {
      __emnapiContextDestroyPromise = undefined
      throw error
    },
  )
  __emnapiContextDestroyPromise = destroyPromise
  return destroyPromise
}

function __terminateWasiWorkers() {
  const cleanupErrors = []
  const pending = []

  for (const worker of __wasiWorkers) {
    let result
    try {
      result = worker.terminate()
    } catch (error) {
      cleanupErrors.push(error)
      continue
    }
    if (__isThenable(result)) {
      pending.push(
        Promise.resolve(result).then(
          () => {
            __wasiWorkers.delete(worker)
          },
          (error) => {
            cleanupErrors.push(error)
          },
        ),
      )
    } else {
      __wasiWorkers.delete(worker)
    }
  }

  const finish = () => {
    if (cleanupErrors.length > 0) {
      throw __createCleanupError(
        cleanupErrors,
        'Failed to terminate WASI workers',
      )
    }
  }
  return pending.length > 0 ? Promise.all(pending).then(finish) : finish()
}

function __finishWasiDisposal() {
  const workerResult = __terminateWasiWorkers()
  if (__isThenable(workerResult)) {
    return Promise.resolve(workerResult).then(__completeWasiDisposal)
  }
  return __completeWasiDisposal()
}

function __startWasiDisposal() {
  const destroyResult = __destroyEmnapiContext()
  if (__isThenable(destroyResult)) {
    return Promise.resolve(destroyResult).then(__finishWasiDisposal)
  }
  return __finishWasiDisposal()
}

/**
 * Disposes this generated WASI binding.
 *
 * Access this function with:
 * binding[Symbol.for('napi.rs.wasi.dispose')]()
 */
function __disposeWasiBinding() {
  if (__wasiDisposePromise) {
    return __wasiDisposePromise
  }
  if (__wasiDisposed) {
    return Promise.resolve()
  }

  let resolveDispose
  let rejectDispose
  const disposePromise = new Promise((resolve, reject) => {
    resolveDispose = resolve
    rejectDispose = reject
  })
  __wasiDisposePromise = disposePromise

  let result
  try {
    result = __startWasiDisposal()
  } catch (error) {
    __wasiDisposePromise = undefined
    rejectDispose(error)
    return disposePromise
  }

  Promise.resolve(result).then(
    (value) => {
      __wasiDisposed = true
      resolveDispose(value)
    },
    (error) => {
      __wasiDisposePromise = undefined
      rejectDispose(error)
    },
  )
  return disposePromise
}

function __publishWasiDispose(exports) {
  Object.defineProperty(exports, __wasiDisposeSymbol, {
    configurable: false,
    enumerable: false,
    value: __disposeWasiBinding,
    writable: false,
  })
}

function __finishWasiInitializationRollback(cleanupErrors) {
  let workerResult
  try {
    workerResult = __terminateWasiWorkers()
  } catch (cleanupError) {
    cleanupErrors.push(cleanupError)
    return cleanupErrors
  }
  if (__isThenable(workerResult)) {
    return Promise.resolve(workerResult)
      .catch((cleanupError) => {
        cleanupErrors.push(cleanupError)
      })
      .then(() => cleanupErrors)
  }
  return cleanupErrors
}

function __rollbackWasiInitialization() {
  const cleanupErrors = []
  let destroyResult
  try {
    destroyResult = __destroyEmnapiContext()
  } catch (cleanupError) {
    cleanupErrors.push(cleanupError)
    return __finishWasiInitializationRollback(cleanupErrors)
  }
  if (__isThenable(destroyResult)) {
    return Promise.resolve(destroyResult)
      .catch((cleanupError) => {
        cleanupErrors.push(cleanupError)
      })
      .then(() => __finishWasiInitializationRollback(cleanupErrors))
  }
  return __finishWasiInitializationRollback(cleanupErrors)
}

let __wasiModule
let __napiModule

try {
  __emnapiContext = __emnapiCreateContext({ autoDestroy: false })
  __emnapiContext.suppressDestroy()
  
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
      __wasiWorkers.add(worker)
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
  __publishWasiDispose(__napiModule.exports)
} catch (error) {
  const cleanupErrors = await __rollbackWasiInitialization()
  throw __attachCleanupErrors(error, cleanupErrors)
}
export default __napiModule.exports
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
export const BindingBundleEndEventData = __napiModule.exports.BindingBundleEndEventData
export const BindingBundleErrorEventData = __napiModule.exports.BindingBundleErrorEventData
export const BindingBundler = __napiModule.exports.BindingBundler
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
export const BindingAttachDebugInfo = __napiModule.exports.BindingAttachDebugInfo
export const BindingBuiltinPluginName = __napiModule.exports.BindingBuiltinPluginName
export const BindingChunkModuleOrderBy = __napiModule.exports.BindingChunkModuleOrderBy
export const BindingErrorStage = __napiModule.exports.BindingErrorStage
export const BindingLogLevel = __napiModule.exports.BindingLogLevel
export const BindingPluginOrder = __napiModule.exports.BindingPluginOrder
export const BindingPropertyReadSideEffects = __napiModule.exports.BindingPropertyReadSideEffects
export const BindingPropertyWriteSideEffects = __napiModule.exports.BindingPropertyWriteSideEffects
export const BindingRebuildStrategy = __napiModule.exports.BindingRebuildStrategy
export const collapseSourcemaps = __napiModule.exports.collapseSourcemaps
export const enhancedTransform = __napiModule.exports.enhancedTransform
export const enhancedTransformSync = __napiModule.exports.enhancedTransformSync
export const FilterTokenKind = __napiModule.exports.FilterTokenKind
export const initTraceSubscriber = __napiModule.exports.initTraceSubscriber
export const registerPlugins = __napiModule.exports.registerPlugins
export const resolveTsconfig = __napiModule.exports.resolveTsconfig
export const shutdownAsyncRuntime = __napiModule.exports.shutdownAsyncRuntime
export const startAsyncRuntime = __napiModule.exports.startAsyncRuntime
