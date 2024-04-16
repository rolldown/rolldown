import {
  instantiateNapiModuleSync as __emnapiInstantiateNapiModuleSync,
  getDefaultContext as __emnapiGetDefaultContext,
  WASI as __WASI,
} from '@napi-rs/wasm-runtime'
import { Volume as __Volume, createFsFromVolume as __createFsFromVolume } from '@napi-rs/wasm-runtime/fs'

import __wasmUrl from './rolldown-binding.wasm32-wasi.wasm?url'

const __fs = __createFsFromVolume(
  __Volume.fromJSON({
    '/': null,
  }),
)

const __wasi = new __WASI({
  version: 'preview1',
  fs: __fs,
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
    return new Worker(new URL('./wasi-worker-browser.mjs', import.meta.url), {
      type: 'module',
    })
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
  __napiInstance.exports['__napi_register__Bundler_struct_0']?.()
  __napiInstance.exports['__napi_register__Bundler_impl_5']?.()
  __napiInstance.exports['__napi_register__BindingInputItem_struct_6']?.()
  __napiInstance.exports['__napi_register__BindingResolveOptions_struct_7']?.()
  __napiInstance.exports['__napi_register__BindingInputOptions_struct_8']?.()
  __napiInstance.exports['__napi_register__BindingOutputOptions_struct_9']?.()
  __napiInstance.exports['__napi_register__BindingPluginContext_struct_10']?.()
  __napiInstance.exports['__napi_register__BindingPluginContext_impl_12']?.()
  __napiInstance.exports['__napi_register__BindingPluginOptions_struct_13']?.()
  __napiInstance.exports['__napi_register__BindingPluginWithIndex_struct_14']?.()
  __napiInstance.exports['__napi_register__BindingHookLoadOutput_struct_15']?.()
  __napiInstance.exports['__napi_register__BindingHookRenderChunkOutput_struct_16']?.()
  __napiInstance.exports['__napi_register__BindingHookResolveIdExtraOptions_struct_17']?.()
  __napiInstance.exports['__napi_register__BindingHookResolveIdOutput_struct_18']?.()
  __napiInstance.exports['__napi_register__BindingPluginContextResolveOptions_struct_19']?.()
  __napiInstance.exports['__napi_register__ParallelJsPluginRegistry_struct_20']?.()
  __napiInstance.exports['__napi_register__ParallelJsPluginRegistry_impl_22']?.()
  __napiInstance.exports['__napi_register__register_plugins_23']?.()
  __napiInstance.exports['__napi_register__BindingOutputAsset_struct_24']?.()
  __napiInstance.exports['__napi_register__BindingOutputAsset_impl_27']?.()
  __napiInstance.exports['__napi_register__BindingOutputChunk_struct_28']?.()
  __napiInstance.exports['__napi_register__BindingOutputChunk_impl_39']?.()
  __napiInstance.exports['__napi_register__BindingOutputs_struct_40']?.()
  __napiInstance.exports['__napi_register__BindingOutputs_impl_43']?.()
  __napiInstance.exports['__napi_register__RenderedChunk_struct_44']?.()
  __napiInstance.exports['__napi_register__BindingRenderedModule_struct_45']?.()
  __napiInstance.exports['__napi_register__AliasItem_struct_46']?.()
}
export const BindingOutputAsset = __napiModule.exports.BindingOutputAsset
export const BindingOutputChunk = __napiModule.exports.BindingOutputChunk
export const BindingOutputs = __napiModule.exports.BindingOutputs
export const BindingPluginContext = __napiModule.exports.BindingPluginContext
export const Bundler = __napiModule.exports.Bundler
export const ParallelJsPluginRegistry = __napiModule.exports.ParallelJsPluginRegistry
export const registerPlugins = __napiModule.exports.registerPlugins
