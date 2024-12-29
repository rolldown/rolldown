mod binding_advanced_chunks_options;
mod binding_pre_rendered_chunk;

use derive_more::Debug;
use napi::Either;
use napi_derive::napi;
use rustc_hash::FxHashMap;

use binding_advanced_chunks_options::BindingAdvancedChunksOptions;
use binding_pre_rendered_chunk::PreRenderedChunk;

use super::plugin::BindingPluginOrParallelJsPluginPlaceholder;
use crate::types::{
  binding_rendered_chunk::RenderedChunk,
  js_callback::{JsCallback, MaybeAsyncJsCallback},
};

pub type AddonOutputOption = MaybeAsyncJsCallback<RenderedChunk, Option<String>>;
pub type ChunkFileNamesOutputOption = Either<String, JsCallback<PreRenderedChunk, String>>;
pub type GlobalsOutputOption = Either<FxHashMap<String, String>, JsCallback<String, String>>;

#[napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingOutputOptions {
  // --- Options Rolldown doesn't need to be supported
  // /** @deprecated Use the "renderDynamicImport" plugin hook instead. */
  // dynamicImportFunction: string | undefined;
  pub name: Option<String>,
  pub asset_file_names: Option<String>,

  #[debug(skip)]
  #[napi(ts_type = "string | ((chunk: PreRenderedChunk) => string)")]
  pub entry_file_names: Option<ChunkFileNamesOutputOption>,
  #[debug(skip)]
  #[napi(ts_type = "string | ((chunk: PreRenderedChunk) => string)")]
  pub chunk_file_names: Option<ChunkFileNamesOutputOption>,
  #[debug(skip)]
  #[napi(ts_type = "string | ((chunk: PreRenderedChunk) => string)")]
  pub css_entry_file_names: Option<ChunkFileNamesOutputOption>,
  #[debug(skip)]
  #[napi(ts_type = "string | ((chunk: PreRenderedChunk) => string)")]
  pub css_chunk_file_names: Option<ChunkFileNamesOutputOption>,

  // amd: NormalizedAmdOptions;
  // assetFileNames: string | ((chunkInfo: PreRenderedAsset) => string);
  #[debug(skip)]
  #[napi(ts_type = "(chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub banner: Option<AddonOutputOption>,
  // chunkFileNames: string | ((chunkInfo: PreRenderedChunk) => string);
  // compact: boolean;
  pub dir: Option<String>,
  pub file: Option<String>,
  // pub entry_file_names: String, // | ((chunkInfo: PreRenderedChunk) => string)
  #[napi(ts_type = "boolean | 'if-default-prop'")]
  pub es_module: Option<Either<bool, String>>,
  #[napi(ts_type = "'default' | 'named' | 'none' | 'auto'")]
  pub exports: Option<String>,
  pub extend: Option<bool>,
  pub external_live_bindings: Option<bool>,
  // footer: () => string | Promise<string>;
  #[debug(skip)]
  #[napi(ts_type = "(chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub footer: Option<AddonOutputOption>,
  #[napi(ts_type = "'es' | 'cjs' | 'iife' | 'umd' | 'app'")]
  pub format: Option<String>,
  // freeze: boolean;
  // generatedCode: NormalizedGeneratedCodeOptions;
  #[debug(skip)]
  #[napi(ts_type = "Record<string, string> | ((name: string) => string)")]
  pub globals: Option<GlobalsOutputOption>,
  #[napi(ts_type = "'base64' | 'base36' | 'hex'")]
  pub hash_characters: Option<String>,
  // hoistTransitiveImports: boolean;
  // indent: true | string;
  pub inline_dynamic_imports: Option<bool>,
  // interop: GetInterop;
  #[debug(skip)]
  #[napi(ts_type = "(chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub intro: Option<AddonOutputOption>,
  // manualChunks: ManualChunksOption;
  // minifyInternalExports: boolean;
  // namespaceToStringTag: boolean;
  // noConflict: boolean;
  #[debug(skip)]
  #[napi(ts_type = "(chunk: RenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub outro: Option<AddonOutputOption>,
  // paths: OptionsPaths;
  #[napi(ts_type = "(BindingBuiltinPlugin | BindingPluginOptions | undefined)[]")]
  pub plugins: Vec<BindingPluginOrParallelJsPluginPlaceholder>,
  // preferConst: boolean;
  // preserveModules: boolean;
  // preserveModulesRoot: string | undefined;
  // sanitizeFileName: (fileName: string) => string;
  #[napi(ts_type = "'file' | 'inline' | 'hidden'")]
  pub sourcemap: Option<String>,
  #[debug(skip)]
  #[napi(ts_type = "(source: string, sourcemapPath: string) => boolean")]
  pub sourcemap_ignore_list: Option<JsCallback<(String, String), bool>>,
  pub sourcemap_debug_ids: Option<bool>,
  #[debug(skip)]
  #[napi(ts_type = "(source: string, sourcemapPath: string) => string")]
  pub sourcemap_path_transform: Option<JsCallback<(String, String), String>>,
  // sourcemapExcludeSources: boolean;
  // sourcemapFile: string | undefined;
  // strict: boolean;
  // systemNullSetters: boolean;
  // validate: boolean;

  // --- Enhanced options
  pub minify: Option<bool>,
  pub advanced_chunks: Option<BindingAdvancedChunksOptions>,
  #[napi(ts_type = "'none' | 'preserve-legal'")]
  pub comments: Option<String>,
  pub polyfill_require: Option<bool>,
  pub target: Option<String>,
}
