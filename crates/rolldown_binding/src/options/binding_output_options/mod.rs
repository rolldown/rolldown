use crate::types::js_callback::MaybeAsyncJsCallback;

use super::super::types::binding_rendered_chunk::RenderedChunk;
use super::plugin::BindingPluginOrParallelJsPluginPlaceholder;
use derivative::Derivative;
use napi::threadsafe_function::ThreadsafeFunction;
use napi_derive::napi;
use serde::Deserialize;

pub type AddonOutputOption = MaybeAsyncJsCallback<RenderedChunk, Option<String>>;

#[napi(object, object_to_js = false)]
#[derive(Deserialize, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingOutputOptions {
  // --- Options Rolldown doesn't need to be supported
  // /** @deprecated Use the "renderDynamicImport" plugin hook instead. */
  // dynamicImportFunction: string | undefined;
  pub entry_file_names: Option<String>,
  pub chunk_file_names: Option<String>,
  pub asset_file_names: Option<String>,
  // amd: NormalizedAmdOptions;
  // assetFileNames: string | ((chunkInfo: PreRenderedAsset) => string);
  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub banner: Option<AddonOutputOption>,
  // chunkFileNames: string | ((chunkInfo: PreRenderedChunk) => string);
  // compact: boolean;
  pub dir: Option<String>,
  // pub entry_file_names: String, // | ((chunkInfo: PreRenderedChunk) => string)
  // esModule: boolean;
  #[napi(ts_type = "'default' | 'named' | 'none' | 'auto'")]
  pub exports: Option<String>,
  // extend: boolean;
  // externalLiveBindings: boolean;
  // footer: () => string | Promise<string>;
  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub footer: Option<AddonOutputOption>,
  #[napi(ts_type = "'es' | 'cjs'")]
  pub format: Option<String>,
  // freeze: boolean;
  // generatedCode: NormalizedGeneratedCodeOptions;
  // globals: GlobalsOption;
  // hoistTransitiveImports: boolean;
  // indent: true | string;
  // inlineDynamicImports: boolean;
  // interop: GetInterop;
  // intro: () => string | Promise<string>;
  // manualChunks: ManualChunksOption;
  // minifyInternalExports: boolean;
  // name: string | undefined;
  // namespaceToStringTag: boolean;
  // noConflict: boolean;
  // outro: () => string | Promise<string>;
  // paths: OptionsPaths;
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(BindingBuiltinPlugin | BindingPluginOptions | undefined)[]")]
  pub plugins: Vec<BindingPluginOrParallelJsPluginPlaceholder>,
  // preferConst: boolean;
  // preserveModules: boolean;
  // preserveModulesRoot: string | undefined;
  // sanitizeFileName: (fileName: string) => string;
  #[napi(ts_type = "'file' | 'inline' | 'hidden'")]
  pub sourcemap: Option<String>,
  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(source: string, sourcemapPath: string) => boolean")]
  pub sourcemap_ignore_list:
    Option<ThreadsafeFunction<(String, String), bool, (String, String), false>>,
  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(source: string, sourcemapPath: string) => string")]
  pub sourcemap_path_transform:
    Option<ThreadsafeFunction<(String, String), String, (String, String), false>>,
  // sourcemapExcludeSources: boolean;
  // sourcemapFile: string | undefined;
  // strict: boolean;
  // systemNullSetters: boolean;
  // validate: boolean;
  // --- Enhanced options
  // pub minify: bool,
}
