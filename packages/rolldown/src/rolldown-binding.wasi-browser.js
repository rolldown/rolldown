import {
  instantiateNapiModuleSync as __emnapiInstantiateNapiModuleSync,
  getDefaultContext as __emnapiGetDefaultContext,
  WASI as __WASI,
  createOnMessage as __wasmCreateOnMessageForFsProxy,
} from '@napi-rs/wasm-runtime'
import { memfs } from '@napi-rs/wasm-runtime/fs'
import __wasmUrl from './rolldown-binding.wasm32-wasi.wasm?url'

export const { fs: __fs, vol: __volume } = memfs()

const __wasi = new __WASI({
  version: 'preview1',
  fs: __fs,
  preopens: {
    '/': '/',
  }
})

const __emnapiContext = __emnapiGetDefaultContext()

const __sharedMemory = new WebAssembly.Memory({
  initial: 16384,
  maximum: 65536,
  shared: true,
})

const __wasmFile = await fetch(__wasmUrl).then((res) => res.arrayBuffer())

const {
  instance: __napiInstance,
  module: __wasiModule,
  napiModule: __napiModule,
} = __emnapiInstantiateNapiModuleSync(__wasmFile, {
  context: __emnapiContext,
  asyncWorkPoolSize: 4,
  wasi: __wasi,
  onCreateWorker() {
    const worker = new Worker(new URL('./wasi-worker-browser.mjs', import.meta.url), {
      type: 'module',
    })
    
    worker.addEventListener('message', __wasmCreateOnMessageForFsProxy(__fs))

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
    __napi_rs_initialize_modules(instance)
  },
})

function __napi_rs_initialize_modules(__napiInstance) {
  __napiInstance.exports['__napi_register__IsolatedDeclarationsResult_struct_0']?.()
  __napiInstance.exports['__napi_register__isolated_declaration_1']?.()
  __napiInstance.exports['__napi_register__TypeScriptBindingOptions_struct_2']?.()
  __napiInstance.exports['__napi_register__ReactBindingOptions_struct_3']?.()
  __napiInstance.exports['__napi_register__ArrowFunctionsBindingOptions_struct_4']?.()
  __napiInstance.exports['__napi_register__ES2015BindingOptions_struct_5']?.()
  __napiInstance.exports['__napi_register__TransformOptions_struct_6']?.()
  __napiInstance.exports['__napi_register__Sourcemap_struct_7']?.()
  __napiInstance.exports['__napi_register__TransformResult_struct_8']?.()
  __napiInstance.exports['__napi_register__transform_9']?.()
  __napiInstance.exports['__napi_register__Bundler_struct_0']?.()
  __napiInstance.exports['__napi_register__Bundler_impl_5']?.()
  __napiInstance.exports['__napi_register__BindingInputItem_struct_6']?.()
  __napiInstance.exports['__napi_register__BindingResolveOptions_struct_7']?.()
  __napiInstance.exports['__napi_register__BindingTreeshake_struct_8']?.()
  __napiInstance.exports['__napi_register__BindingInputOptions_struct_9']?.()
  __napiInstance.exports['__napi_register__BindingOutputOptions_struct_10']?.()
  __napiInstance.exports['__napi_register__BindingPluginContext_struct_11']?.()
  __napiInstance.exports['__napi_register__BindingPluginContext_impl_17']?.()
  __napiInstance.exports['__napi_register__BindingPluginContextResolvedId_struct_18']?.()
  __napiInstance.exports['__napi_register__BindingPluginOptions_struct_19']?.()
  __napiInstance.exports['__napi_register__BindingPluginWithIndex_struct_20']?.()
  __napiInstance.exports['__napi_register__BindingTransformPluginContext_struct_21']?.()
  __napiInstance.exports['__napi_register__BindingTransformPluginContext_impl_23']?.()
  __napiInstance.exports['__napi_register__BindingAssetSource_struct_24']?.()
  __napiInstance.exports['__napi_register__BindingEmittedAsset_struct_25']?.()
  __napiInstance.exports['__napi_register__BindingHookLoadOutput_struct_26']?.()
  __napiInstance.exports['__napi_register__BindingHookRenderChunkOutput_struct_27']?.()
  __napiInstance.exports['__napi_register__BindingHookResolveIdExtraOptions_struct_28']?.()
  __napiInstance.exports['__napi_register__BindingHookResolveIdOutput_struct_29']?.()
  __napiInstance.exports['__napi_register__BindingHookSideEffects_30']?.()
  __napiInstance.exports['__napi_register__BindingHookTransformOutput_struct_31']?.()
  __napiInstance.exports['__napi_register__BindingPluginContextResolveOptions_struct_32']?.()
  __napiInstance.exports['__napi_register__BindingBuiltinPlugin_struct_33']?.()
  __napiInstance.exports['__napi_register__BindingBuiltinPluginName_34']?.()
  __napiInstance.exports['__napi_register__ParallelJsPluginRegistry_struct_35']?.()
  __napiInstance.exports['__napi_register__ParallelJsPluginRegistry_impl_37']?.()
  __napiInstance.exports['__napi_register__register_plugins_38']?.()
  __napiInstance.exports['__napi_register__BindingLog_struct_39']?.()
  __napiInstance.exports['__napi_register__BindingLogLevel_40']?.()
  __napiInstance.exports['__napi_register__BindingModuleInfo_struct_41']?.()
  __napiInstance.exports['__napi_register__BindingModuleInfo_impl_43']?.()
  __napiInstance.exports['__napi_register__BindingOutputAsset_struct_44']?.()
  __napiInstance.exports['__napi_register__BindingOutputAsset_impl_48']?.()
  __napiInstance.exports['__napi_register__BindingOutputChunk_struct_49']?.()
  __napiInstance.exports['__napi_register__BindingOutputChunk_impl_67']?.()
  __napiInstance.exports['__napi_register__BindingOutputs_struct_68']?.()
  __napiInstance.exports['__napi_register__BindingOutputs_impl_72']?.()
  __napiInstance.exports['__napi_register__FinalBindingOutputs_struct_73']?.()
  __napiInstance.exports['__napi_register__FinalBindingOutputs_impl_76']?.()
  __napiInstance.exports['__napi_register__RenderedChunk_struct_77']?.()
  __napiInstance.exports['__napi_register__BindingRenderedModule_struct_78']?.()
  __napiInstance.exports['__napi_register__AliasItem_struct_79']?.()
  __napiInstance.exports['__napi_register__BindingSourcemap_struct_80']?.()
  __napiInstance.exports['__napi_register__BindingJSONSourcemap_struct_81']?.()
}
export const BindingLog = __napiModule.exports.BindingLog
export const BindingModuleInfo = __napiModule.exports.BindingModuleInfo
export const BindingOutputAsset = __napiModule.exports.BindingOutputAsset
export const BindingOutputChunk = __napiModule.exports.BindingOutputChunk
export const BindingOutputs = __napiModule.exports.BindingOutputs
export const BindingPluginContext = __napiModule.exports.BindingPluginContext
export const BindingTransformPluginContext = __napiModule.exports.BindingTransformPluginContext
export const Bundler = __napiModule.exports.Bundler
export const FinalBindingOutputs = __napiModule.exports.FinalBindingOutputs
export const ParallelJsPluginRegistry = __napiModule.exports.ParallelJsPluginRegistry
export const BindingBuiltinPluginName = __napiModule.exports.BindingBuiltinPluginName
export const BindingHookSideEffects = __napiModule.exports.BindingHookSideEffects
export const BindingLogLevel = __napiModule.exports.BindingLogLevel
export const isolatedDeclaration = __napiModule.exports.isolatedDeclaration
export const registerPlugins = __napiModule.exports.registerPlugins
export const transform = __napiModule.exports.transform
