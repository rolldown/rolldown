import {
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

const __wasmUrl = new URL('./rolldown-binding.wasm32-wasip1.wasm', import.meta.url).href
const __wasmResponse = await globalThis.fetch(__wasmUrl)
if (!__wasmResponse.ok) {
  throw new Error(
    'Failed to fetch WASI module ' + __wasmUrl + ': ' +
      __wasmResponse.status + ' ' +
      (__wasmResponse.statusText || 'Unknown Status'),
  )
}
const __wasmFile = await __wasmResponse.arrayBuffer()

const __wasmMemory = new WebAssembly.Memory({
  initial: 1024,
  maximum: 65536,
})

const __emnapiContext = __emnapiCreateContext()

function __createInitializationCleanupError(__error, __cleanupError) {
  let __message = 'WASI module initialization failed'
  try {
    if (__error && typeof __error.message === 'string') {
      __message = __error.message
    }
  } catch {}
  const __errors = [__error, __cleanupError]
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

let __browserTaskHostRegistration
let __browserTimerHostRegistration
let __napiInstance
let __wasiModule
let __napiModule

try {
/* ROLLDOWN_BROWSER_INITIALIZATION_GUARD_START */
  __emnapiContext.feature.Buffer = Buffer

  ;({
    instance: __napiInstance,
    module: __wasiModule,
    napiModule: __napiModule,
  } = await __emnapiInstantiateNapiModule(__wasmFile, {
    context: __emnapiContext,
    asyncWorkPoolSize: 0,
    wasi: __wasi,
    overwriteImports(importObject) {
      importObject.env = {
        ...importObject.env,
        ...importObject.napi,
        ...importObject.emnapi,
        memory: __wasmMemory,
      }
      return importObject
    },
    beforeInit({ instance }) {
      for (const name of Object.keys(instance.exports)) {
        if (name.startsWith('__napi_register__')) {
          instance.exports[name]()
        }
      }
    },
  }))
/* ROLLDOWN_CURRENT_THREAD_HOST_BOOTSTRAP_START */
;{
  const __rolldownBinding = __napiModule.exports
  const __getCurrentThreadTaskHostContractVersion =
    __rolldownBinding.getCurrentThreadTaskHostContractVersion
  const __registerCurrentThreadTaskHost =
    __rolldownBinding.registerCurrentThreadTaskHost
  const __registerTimerHost = __rolldownBinding.registerTimerHost
  const __unregisterCurrentThreadTaskHost =
    __rolldownBinding.unregisterCurrentThreadTaskHost
  const __unregisterTimerHost = __rolldownBinding.unregisterTimerHost
  if (
    typeof __getCurrentThreadTaskHostContractVersion !== 'function' ||
    typeof __registerCurrentThreadTaskHost !== 'function' ||
    typeof __registerTimerHost !== 'function' ||
    typeof __unregisterCurrentThreadTaskHost !== 'function' ||
    typeof __unregisterTimerHost !== 'function'
  ) {
    throw new TypeError(
      'The threadless Rolldown binding does not expose its CurrentThread host integration',
    )
  }
  const __taskHostContractVersion =
    Reflect.apply(
      __getCurrentThreadTaskHostContractVersion,
      __rolldownBinding,
      [],
    )
  if (__taskHostContractVersion !== 2) {
    throw new TypeError(
      'The threadless Rolldown binding uses CurrentThread task-host contract version ' +
        String(__taskHostContractVersion) +
        ', but version 2 is required',
    )
  }
  const __readHostRegistration = (__registration, __label) => {
    let __high
    let __low
    try {
      __high = Reflect.get(__registration, 'high', __registration)
      __low = Reflect.get(__registration, 'low', __registration)
    } catch {}
    if (
      !Number.isInteger(__high) ||
      __high < 0 ||
      __high > 0xffffffff ||
      !Number.isInteger(__low) ||
      __low < 0 ||
      __low > 0xffffffff ||
      (__high === 0 && __low === 0)
    ) {
      throw new TypeError(
        'The threadless Rolldown binding returned an invalid ' +
          __label +
          ' host registration',
      )
    }
    return { high: __high, low: __low }
  }
  const __taskHostRegistration = __readHostRegistration(
    Reflect.apply(__registerCurrentThreadTaskHost, __rolldownBinding, []),
    'task',
  )
  __browserTaskHostRegistration = __taskHostRegistration

  const __setTimeoutHost = globalThis.setTimeout?.bind(globalThis)
  const __clearTimeoutHost = globalThis.clearTimeout?.bind(globalThis)
  if (__setTimeoutHost && __clearTimeoutHost) {
    const __MAX_HOST_TIMEOUT_MS = 2147483647
    const __activeTimers = new Map()
    const __armTimer = (__id, __timer) => {
      const __delay = Math.min(__timer.remainingMs, __MAX_HOST_TIMEOUT_MS)
      __timer.handle = __setTimeoutHost(() => {
        if (__activeTimers.get(__id) !== __timer) return
        __timer.remainingMs -= __delay
        if (__timer.remainingMs > 0) {
          try {
            __armTimer(__id, __timer)
          } catch (__error) {
            __activeTimers.delete(__id)
            __timer.reject(__error)
          }
          return
        }
        __activeTimers.delete(__id)
        __timer.resolve()
      }, __delay)
    }
    const __cancelTimer = (__timer) => {
      try {
        if (__timer.handle !== undefined) {
          __clearTimeoutHost(__timer.handle)
        }
      } catch {
        // Rust invokes this callback through a non-catching TSFN. Contain
        // host cancellation failures at the JavaScript boundary.
      } finally {
        __timer.resolve()
      }
    }
    const __timerHostRegistration = __readHostRegistration(
      Reflect.apply(__registerTimerHost, __rolldownBinding, [
        (__id, __ms) => {
          const __previous = __activeTimers.get(__id)
          if (__previous) {
            __activeTimers.delete(__id)
            __cancelTimer(__previous)
          }
          return new Promise((__resolve, __reject) => {
            const __timer = {
              handle: undefined,
              remainingMs: Math.max(__ms, 0),
              reject: __reject,
              resolve: __resolve,
            }
            __activeTimers.set(__id, __timer)
            try {
              __armTimer(__id, __timer)
            } catch (__error) {
              if (__activeTimers.get(__id) === __timer) {
                __activeTimers.delete(__id)
              }
              __reject(__error)
            }
          })
        },
        (__id) => {
          const __timer = __activeTimers.get(__id)
          if (!__timer) return
          __activeTimers.delete(__id)
          __cancelTimer(__timer)
        },
      ]),
      'timer',
    )
    __browserTimerHostRegistration = __timerHostRegistration
  }
}
/* ROLLDOWN_CURRENT_THREAD_HOST_BOOTSTRAP_END */
/* ROLLDOWN_BROWSER_INITIALIZATION_GUARD_END */
} catch (__error) {
  const __cleanupErrors = []
  const __cleanupSync = (__operation, __message) => {
    const __operationErrors = []
    for (let __attempt = 0; __attempt < 2; __attempt += 1) {
      try {
        __operation()
        return
      } catch (__cleanupError) {
        __operationErrors.push(__cleanupError)
      }
    }
    __cleanupErrors.push(new AggregateError(__operationErrors, __message))
  }
  const __cleanup = async (__operation, __message) => {
    const __operationErrors = []
    for (let __attempt = 0; __attempt < 2; __attempt += 1) {
      try {
        await __operation()
        return
      } catch (__cleanupError) {
        __operationErrors.push(__cleanupError)
      }
    }
    __cleanupErrors.push(new AggregateError(__operationErrors, __message))
  }
  if (__browserTimerHostRegistration !== undefined) {
    __cleanupSync(() => {
      const __binding = __napiModule.exports
      Reflect.apply(__binding.unregisterTimerHost, __binding, [
        __browserTimerHostRegistration.high,
        __browserTimerHostRegistration.low,
      ])
    }, 'Threadless browser timer-host cleanup failed')
  }
  if (__browserTaskHostRegistration !== undefined) {
    __cleanupSync(() => {
      const __binding = __napiModule.exports
      Reflect.apply(__binding.unregisterCurrentThreadTaskHost, __binding, [
        __browserTaskHostRegistration.high,
        __browserTaskHostRegistration.low,
      ])
    }, 'Threadless browser task-host cleanup failed')
  }
  if (__emnapiContext !== undefined) {
    await __cleanup(
      () => __emnapiContext.destroy(),
      'Threadless browser context cleanup failed',
    )
  }
  if (__cleanupErrors.length > 0) {
    throw new AggregateError(
      [
        __error,
        new AggregateError(
          __cleanupErrors,
          'Threadless browser initialization cleanup failed',
        ),
      ],
      'Threadless browser initialization failed and cleanup did not complete',
      { cause: __error },
    )
  }
  throw __error
}
export default __napiModule.exports
export const __rolldownBindingTarget = 'wasi'
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
export const registerCurrentThreadTaskHost = __napiModule.exports.registerCurrentThreadTaskHost
export const registerPlugins = __napiModule.exports.registerPlugins
export const registerTimerHost = __napiModule.exports.registerTimerHost
export const resetAsyncRuntimeMetrics = __napiModule.exports.resetAsyncRuntimeMetrics
export const resolveTsconfig = __napiModule.exports.resolveTsconfig
export const shutdownAsyncRuntime = __napiModule.exports.shutdownAsyncRuntime
export const startAsyncRuntime = __napiModule.exports.startAsyncRuntime
export const unregisterCurrentThreadTaskHost = __napiModule.exports.unregisterCurrentThreadTaskHost
export const unregisterTimerHost = __napiModule.exports.unregisterTimerHost
