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
  __napiInstance.exports['__napi_register__SourceMap_struct_0']?.()
  __napiInstance.exports['__napi_register__OxcError_struct_0']?.()
  __napiInstance.exports['__napi_register__ErrorLabel_struct_1']?.()
  __napiInstance.exports['__napi_register__Severity_2']?.()
  __napiInstance.exports['__napi_register__IsolatedDeclarationsResult_struct_0']?.()
  __napiInstance.exports['__napi_register__IsolatedDeclarationsOptions_struct_1']?.()
  __napiInstance.exports['__napi_register__isolated_declaration_2']?.()
  __napiInstance.exports['__napi_register__TransformResult_struct_3']?.()
  __napiInstance.exports['__napi_register__TransformOptions_struct_4']?.()
  __napiInstance.exports['__napi_register__CompilerAssumptions_struct_5']?.()
  __napiInstance.exports['__napi_register__TypeScriptOptions_struct_6']?.()
  __napiInstance.exports['__napi_register__JsxOptions_struct_7']?.()
  __napiInstance.exports['__napi_register__ReactRefreshOptions_struct_8']?.()
  __napiInstance.exports['__napi_register__ArrowFunctionsOptions_struct_9']?.()
  __napiInstance.exports['__napi_register__Es2015Options_struct_10']?.()
  __napiInstance.exports['__napi_register__Helpers_struct_11']?.()
  __napiInstance.exports['__napi_register__HelperMode_12']?.()
  __napiInstance.exports['__napi_register__transform_13']?.()
  __napiInstance.exports['__napi_register__BindingBundlerOptions_struct_0']?.()
  __napiInstance.exports['__napi_register__Bundler_struct_1']?.()
  __napiInstance.exports['__napi_register__Bundler_impl_8']?.()
  __napiInstance.exports['__napi_register__BindingChecksOptions_struct_9']?.()
  __napiInstance.exports['__napi_register__BindingExperimentalOptions_struct_10']?.()
  __napiInstance.exports['__napi_register__BindingInjectImportNamed_struct_11']?.()
  __napiInstance.exports['__napi_register__BindingInjectImportNamespace_struct_12']?.()
  __napiInstance.exports['__napi_register__BindingInputItem_struct_13']?.()
  __napiInstance.exports['__napi_register__BindingWatchOption_struct_14']?.()
  __napiInstance.exports['__napi_register__BindingResolveOptions_struct_15']?.()
  __napiInstance.exports['__napi_register__BindingTreeshake_struct_16']?.()
  __napiInstance.exports['__napi_register__BindingModuleSideEffectsRule_struct_17']?.()
  __napiInstance.exports['__napi_register__BindingInputOptions_struct_18']?.()
  __napiInstance.exports['__napi_register__BindingAdvancedChunksOptions_struct_19']?.()
  __napiInstance.exports['__napi_register__BindingMatchGroup_struct_20']?.()
  __napiInstance.exports['__napi_register__BindingOutputOptions_struct_21']?.()
  __napiInstance.exports['__napi_register__BindingPluginContext_struct_22']?.()
  __napiInstance.exports['__napi_register__BindingPluginContext_impl_30']?.()
  __napiInstance.exports['__napi_register__BindingPluginContextResolvedId_struct_31']?.()
  __napiInstance.exports['__napi_register__BindingPluginOptions_struct_32']?.()
  __napiInstance.exports['__napi_register__BindingPluginWithIndex_struct_33']?.()
  __napiInstance.exports['__napi_register__BindingTransformPluginContext_struct_34']?.()
  __napiInstance.exports['__napi_register__BindingTransformPluginContext_impl_37']?.()
  __napiInstance.exports['__napi_register__BindingAssetSource_struct_38']?.()
  __napiInstance.exports['__napi_register__BindingBuiltinPluginName_39']?.()
  __napiInstance.exports['__napi_register__BindingEmittedAsset_struct_40']?.()
  __napiInstance.exports['__napi_register__BindingGeneralHookFilter_struct_41']?.()
  __napiInstance.exports['__napi_register__BindingTransformHookFilter_struct_42']?.()
  __napiInstance.exports['__napi_register__BindingHookLoadOutput_struct_43']?.()
  __napiInstance.exports['__napi_register__BindingHookRenderChunkOutput_struct_44']?.()
  __napiInstance.exports['__napi_register__BindingHookResolveIdExtraArgs_struct_45']?.()
  __napiInstance.exports['__napi_register__BindingHookResolveIdOutput_struct_46']?.()
  __napiInstance.exports['__napi_register__BindingHookSideEffects_47']?.()
  __napiInstance.exports['__napi_register__BindingHookTransformOutput_struct_48']?.()
  __napiInstance.exports['__napi_register__BindingPluginContextResolveOptions_struct_49']?.()
  __napiInstance.exports['__napi_register__BindingTransformHookExtraArgs_struct_50']?.()
  __napiInstance.exports['__napi_register__BindingBuiltinPlugin_struct_51']?.()
  __napiInstance.exports['__napi_register__BindingGlobImportPluginConfig_struct_52']?.()
  __napiInstance.exports['__napi_register__BindingManifestPluginConfig_struct_53']?.()
  __napiInstance.exports['__napi_register__BindingModulePreloadPolyfillPluginConfig_struct_54']?.()
  __napiInstance.exports['__napi_register__BindingJsonPluginConfig_struct_55']?.()
  __napiInstance.exports['__napi_register__BindingJsonPluginStringify_struct_56']?.()
  __napiInstance.exports['__napi_register__BindingTransformPluginConfig_struct_57']?.()
  __napiInstance.exports['__napi_register__BindingAliasPluginConfig_struct_58']?.()
  __napiInstance.exports['__napi_register__BindingDenoLoaderPluginConfig_struct_59']?.()
  __napiInstance.exports['__napi_register__BindingAliasPluginAlias_struct_60']?.()
  __napiInstance.exports['__napi_register__BindingBuildImportAnalysisPluginConfig_struct_61']?.()
  __napiInstance.exports['__napi_register__BindingViteResolvePluginConfig_struct_62']?.()
  __napiInstance.exports['__napi_register__BindingViteResolvePluginResolveOptions_struct_63']?.()
  __napiInstance.exports['__napi_register__BindingReplacePluginConfig_struct_64']?.()
  __napiInstance.exports['__napi_register__BindingCallableBuiltinPlugin_struct_65']?.()
  __napiInstance.exports['__napi_register__BindingCallableBuiltinPlugin_impl_70']?.()
  __napiInstance.exports['__napi_register__BindingHookJsResolveIdOptions_struct_71']?.()
  __napiInstance.exports['__napi_register__BindingHookJsResolveIdOutput_struct_72']?.()
  __napiInstance.exports['__napi_register__BindingHookJsLoadOutput_struct_73']?.()
  __napiInstance.exports['__napi_register__BindingJsWatchChangeEvent_struct_74']?.()
  __napiInstance.exports['__napi_register__BindingPluginOrder_75']?.()
  __napiInstance.exports['__napi_register__BindingPluginHookMeta_struct_76']?.()
  __napiInstance.exports['__napi_register__ParallelJsPluginRegistry_struct_77']?.()
  __napiInstance.exports['__napi_register__ParallelJsPluginRegistry_impl_79']?.()
  __napiInstance.exports['__napi_register__register_plugins_80']?.()
  __napiInstance.exports['__napi_register__BindingLog_struct_81']?.()
  __napiInstance.exports['__napi_register__BindingLogLevel_82']?.()
  __napiInstance.exports['__napi_register__BindingModuleInfo_struct_83']?.()
  __napiInstance.exports['__napi_register__BindingModuleInfo_impl_85']?.()
  __napiInstance.exports['__napi_register__BindingNormalizedOptions_struct_86']?.()
  __napiInstance.exports['__napi_register__BindingNormalizedOptions_impl_116']?.()
  __napiInstance.exports['__napi_register__BindingOutputAsset_struct_117']?.()
  __napiInstance.exports['__napi_register__BindingOutputAsset_impl_124']?.()
  __napiInstance.exports['__napi_register__JsOutputAsset_struct_125']?.()
  __napiInstance.exports['__napi_register__BindingOutputChunk_struct_126']?.()
  __napiInstance.exports['__napi_register__BindingOutputChunk_impl_141']?.()
  __napiInstance.exports['__napi_register__JsOutputChunk_struct_142']?.()
  __napiInstance.exports['__napi_register__BindingOutputs_struct_143']?.()
  __napiInstance.exports['__napi_register__BindingOutputs_impl_147']?.()
  __napiInstance.exports['__napi_register__JsChangedOutputs_struct_148']?.()
  __napiInstance.exports['__napi_register__BindingError_struct_149']?.()
  __napiInstance.exports['__napi_register__PreRenderedChunk_struct_150']?.()
  __napiInstance.exports['__napi_register__RenderedChunk_struct_151']?.()
  __napiInstance.exports['__napi_register__BindingRenderedModule_struct_152']?.()
  __napiInstance.exports['__napi_register__BindingRenderedModule_impl_154']?.()
  __napiInstance.exports['__napi_register__AliasItem_struct_155']?.()
  __napiInstance.exports['__napi_register__ExtensionAliasItem_struct_156']?.()
  __napiInstance.exports['__napi_register__BindingSourcemap_struct_157']?.()
  __napiInstance.exports['__napi_register__BindingJsonSourcemap_struct_158']?.()
  __napiInstance.exports['__napi_register__BindingWatcherEvent_struct_159']?.()
  __napiInstance.exports['__napi_register__BindingWatcherEvent_impl_165']?.()
  __napiInstance.exports['__napi_register__BindingWatcherChangeData_struct_166']?.()
  __napiInstance.exports['__napi_register__BindingBundleEndEventData_struct_167']?.()
  __napiInstance.exports['__napi_register__BindingNotifyOption_struct_168']?.()
  __napiInstance.exports['__napi_register__BindingWatcher_struct_169']?.()
  __napiInstance.exports['__napi_register__BindingWatcher_impl_173']?.()
}
export const BindingBundleEndEventData = __napiModule.exports.BindingBundleEndEventData
export const BindingCallableBuiltinPlugin = __napiModule.exports.BindingCallableBuiltinPlugin
export const BindingError = __napiModule.exports.BindingError
export const BindingLog = __napiModule.exports.BindingLog
export const BindingModuleInfo = __napiModule.exports.BindingModuleInfo
export const BindingNormalizedOptions = __napiModule.exports.BindingNormalizedOptions
export const BindingOutputAsset = __napiModule.exports.BindingOutputAsset
export const BindingOutputChunk = __napiModule.exports.BindingOutputChunk
export const BindingOutputs = __napiModule.exports.BindingOutputs
export const BindingPluginContext = __napiModule.exports.BindingPluginContext
export const BindingRenderedModule = __napiModule.exports.BindingRenderedModule
export const BindingTransformPluginContext = __napiModule.exports.BindingTransformPluginContext
export const BindingWatcher = __napiModule.exports.BindingWatcher
export const BindingWatcherChangeData = __napiModule.exports.BindingWatcherChangeData
export const BindingWatcherEvent = __napiModule.exports.BindingWatcherEvent
export const Bundler = __napiModule.exports.Bundler
export const ParallelJsPluginRegistry = __napiModule.exports.ParallelJsPluginRegistry
export const BindingBuiltinPluginName = __napiModule.exports.BindingBuiltinPluginName
export const BindingHookSideEffects = __napiModule.exports.BindingHookSideEffects
export const BindingLogLevel = __napiModule.exports.BindingLogLevel
export const BindingPluginOrder = __napiModule.exports.BindingPluginOrder
export const HelperMode = __napiModule.exports.HelperMode
export const isolatedDeclaration = __napiModule.exports.isolatedDeclaration
export const registerPlugins = __napiModule.exports.registerPlugins
export const Severity = __napiModule.exports.Severity
export const transform = __napiModule.exports.transform
