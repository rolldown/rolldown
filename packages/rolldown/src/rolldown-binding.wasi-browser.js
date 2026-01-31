import {
  createOnMessage as __wasmCreateOnMessageForFsProxy,
  getDefaultContext as __emnapiGetDefaultContext,
  instantiateNapiModule as __emnapiInstantiateNapiModule,
  WASI as __WASI,
} from '@napi-rs/wasm-runtime'
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
const __emnapiContext = __emnapiGetDefaultContext()


const __sharedMemory = new WebAssembly.Memory({
  initial: 16384,
  maximum: 65536,
  shared: true,
})

const __wasmFile = await fetch(__wasmUrl).then((res) => res.arrayBuffer())

// Check if WebAssembly.Module can be cloned (Safari doesn't support this)
let __supportsModuleClone = false
try {
  const testModule = new WebAssembly.Module(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]))
  new MessageChannel().port1.postMessage(testModule)
  __supportsModuleClone = true
} catch {
  // Safari throws DataCloneError
  __supportsModuleClone = false
}

const {
  instance: __napiInstance,
  module: __wasiModule,
  napiModule: __napiModule,
} = await __emnapiInstantiateNapiModule(__wasmFile, {
  context: __emnapiContext,
  asyncWorkPoolSize: 4,
  wasi: __wasi,
  onCreateWorker() {
    const worker = new Worker(new URL('./wasi-worker-browser.mjs', import.meta.url), {
      type: 'module',
    })
    worker.addEventListener('message', __wasmCreateOnMessageForFsProxy(__fs))
    
    // Store info about module clone support on worker
    worker.__supportsModuleClone = __supportsModuleClone
    worker.__wasmFile = __wasmFile

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
    for (const name of Object.keys(instance.exports)) {
      if (name.startsWith('__napi_register__')) {
        instance.exports[name]()
      }
    }
  },
})

// Patch PThread.loadWasmModuleToWorker to send raw bytes on Safari
if (!__supportsModuleClone && __napiModule.PThread) {
  const originalLoadWasmModuleToWorker = __napiModule.PThread.loadWasmModuleToWorker
  __napiModule.PThread.loadWasmModuleToWorker = function(worker, sab) {
    // Intercept and send raw bytes instead of module
    const originalPostMessage = worker.postMessage
    worker.postMessage = function(message) {
      if (message && message.__emnapi__ && message.__emnapi__.type === 'load') {
        // Replace wasmModule with raw bytes
        const modifiedMessage = {
          ...message,
          __emnapi__: {
            ...message.__emnapi__,
            payload: {
              ...message.__emnapi__.payload,
              wasmModule: null,
              wasmBytes: worker.__wasmFile,
              wasmMemory: message.__emnapi__.payload.wasmMemory
            }
          }
        }
        return originalPostMessage.call(this, modifiedMessage)
      }
      return originalPostMessage.apply(this, arguments)
    }
    
    const result = originalLoadWasmModuleToWorker.call(this, worker, sab)
    
    // Restore original postMessage after load
    worker.postMessage = originalPostMessage
    
    return result
  }
}
export default __napiModule.exports
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
export const ScheduledBuild = __napiModule.exports.ScheduledBuild
export const TraceSubscriberGuard = __napiModule.exports.TraceSubscriberGuard
export const BindingAttachDebugInfo = __napiModule.exports.BindingAttachDebugInfo
export const BindingBuiltinPluginName = __napiModule.exports.BindingBuiltinPluginName
export const BindingChunkModuleOrderBy = __napiModule.exports.BindingChunkModuleOrderBy
export const BindingLogLevel = __napiModule.exports.BindingLogLevel
export const BindingPluginOrder = __napiModule.exports.BindingPluginOrder
export const BindingPropertyReadSideEffects = __napiModule.exports.BindingPropertyReadSideEffects
export const BindingPropertyWriteSideEffects = __napiModule.exports.BindingPropertyWriteSideEffects
export const BindingRebuildStrategy = __napiModule.exports.BindingRebuildStrategy
export const collapseSourcemaps = __napiModule.exports.collapseSourcemaps
export const createTokioRuntime = __napiModule.exports.createTokioRuntime
export const FilterTokenKind = __napiModule.exports.FilterTokenKind
export const initTraceSubscriber = __napiModule.exports.initTraceSubscriber
export const registerPlugins = __napiModule.exports.registerPlugins
export const shutdownAsyncRuntime = __napiModule.exports.shutdownAsyncRuntime
export const startAsyncRuntime = __napiModule.exports.startAsyncRuntime
