pub mod binding_advanced_chunks_options;
mod binding_pre_rendered_asset;
mod binding_pre_rendered_chunk;
use binding_pre_rendered_asset::BindingPreRenderedAsset;
use derive_more::Debug;
use napi::Either;
use napi::bindgen_prelude::{Either3, FnArgs};
use napi_derive::napi;
use rustc_hash::FxHashMap;

use binding_advanced_chunks_options::BindingAdvancedChunksOptions;
use binding_pre_rendered_chunk::PreRenderedChunk;

use super::plugin::BindingPluginOrParallelJsPluginPlaceholder;
use crate::types::binding_minify_options::BindingMinifyOptions;
use crate::types::{
  binding_rendered_chunk::BindingRenderedChunk,
  js_callback::{JsCallback, MaybeAsyncJsCallback},
};

pub type AddonOutputOption = MaybeAsyncJsCallback<FnArgs<(BindingRenderedChunk,)>, Option<String>>;
pub type ChunkFileNamesOutputOption =
  Either<String, JsCallback<FnArgs<(PreRenderedChunk,)>, String>>;
pub type AssetFileNamesOutputOption =
  Either<String, JsCallback<FnArgs<(BindingPreRenderedAsset,)>, String>>;
pub type GlobalsOutputOption =
  Either<FxHashMap<String, String>, JsCallback<FnArgs<(String,)>, String>>;
pub type SanitizeFileName = Either<bool, JsCallback<FnArgs<(String,)>, String>>;

#[napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingOutputOptions<'env> {
  // --- Options Rolldown doesn't need to be supported
  // /** @deprecated Use the "renderDynamicImport" plugin hook instead. */
  // dynamicImportFunction: string | undefined;
  pub name: Option<String>,
  #[debug(skip)]
  #[napi(ts_type = "string | ((chunk: BindingPreRenderedAsset) => string)")]
  pub asset_file_names: Option<AssetFileNamesOutputOption>,

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
  #[debug(skip)]
  #[napi(ts_type = "boolean | ((name: string) => string)")]
  pub sanitize_file_name: Option<SanitizeFileName>,
  // amd: NormalizedAmdOptions;
  #[debug(skip)]
  #[napi(ts_type = "(chunk: BindingRenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub banner: Option<AddonOutputOption>,
  // compact: boolean;
  pub dir: Option<String>,
  pub file: Option<String>,
  #[napi(ts_type = "boolean | 'if-default-prop'")]
  pub es_module: Option<Either<bool, String>>,
  #[napi(ts_type = "'default' | 'named' | 'none' | 'auto'")]
  pub exports: Option<String>,
  pub extend: Option<bool>,
  pub external_live_bindings: Option<bool>,
  #[debug(skip)]
  #[napi(ts_type = "(chunk: BindingRenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub footer: Option<AddonOutputOption>,
  #[napi(ts_type = "'es' | 'cjs' | 'iife' | 'umd'")]
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
  #[napi(ts_type = "(chunk: BindingRenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub intro: Option<AddonOutputOption>,
  // manualChunks: ManualChunksOption;
  // minifyInternalExports: boolean;
  // namespaceToStringTag: boolean;
  // noConflict: boolean;
  #[debug(skip)]
  #[napi(ts_type = "(chunk: BindingRenderedChunk) => MaybePromise<VoidNullable<string>>")]
  pub outro: Option<AddonOutputOption>,
  // paths: OptionsPaths;
  #[napi(ts_type = "(BindingBuiltinPlugin | BindingPluginOptions | undefined)[]")]
  pub plugins: Vec<BindingPluginOrParallelJsPluginPlaceholder<'env>>,
  // preferConst: boolean;
  #[napi(ts_type = "'file' | 'inline' | 'hidden'")]
  pub sourcemap: Option<String>,
  pub sourcemap_base_url: Option<String>,
  #[debug(skip)]
  #[napi(ts_type = "(source: string, sourcemapPath: string) => boolean")]
  pub sourcemap_ignore_list: Option<JsCallback<FnArgs<(String, String)>, bool>>,
  pub sourcemap_debug_ids: Option<bool>,
  pub sourcemap_exclude_sources: Option<bool>,
  #[debug(skip)]
  #[napi(ts_type = "(source: string, sourcemapPath: string) => string")]
  pub sourcemap_path_transform: Option<JsCallback<FnArgs<(String, String)>, String>>,
  // sourcemapExcludeSources: boolean;
  // sourcemapFile: string | undefined;
  // strict: boolean;
  // systemNullSetters: boolean;
  // validate: boolean;

  // --- Enhanced options
  #[napi(ts_type = "boolean | 'dce-only' | BindingMinifyOptions")]
  pub minify: Option<Either3<bool, String, BindingMinifyOptions>>,
  pub advanced_chunks: Option<BindingAdvancedChunksOptions>,
  #[napi(ts_type = "'none' | 'inline'")]
  pub legal_comments: Option<String>,
  pub polyfill_require: Option<bool>,
  pub preserve_modules: Option<bool>,
  pub virtual_dirname: Option<String>,
  pub preserve_modules_root: Option<String>,
  pub top_level_var: Option<bool>,
  pub minify_internal_exports: Option<bool>,
}
