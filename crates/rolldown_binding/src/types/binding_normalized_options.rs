use std::collections::HashMap;

use napi::{
  Either,
  bindgen_prelude::{Either3, Undefined},
};
use napi_derive::napi;
use rolldown::{MinifyOptions, SharedNormalizedBundlerOptions};
use rustc_hash::FxBuildHasher;

use crate::utils::minify_options_conversion::{
  codegen_options_to_napi_codegen_options, compress_options_to_napi_compress_options,
  mangle_options_to_napi_mangle_options,
};

#[napi]
pub struct BindingNormalizedOptions {
  inner: SharedNormalizedBundlerOptions,
}

#[napi]
impl BindingNormalizedOptions {
  pub fn new(inner: SharedNormalizedBundlerOptions) -> Self {
    Self { inner }
  }

  // Notice: rust's HashMap doesn't guarantee the order of keys, so not sure if it's a good idea to expose it to JS directly.
  #[napi(getter)]
  pub fn input(&self) -> Either<Vec<String>, HashMap<String, String, FxBuildHasher>> {
    let mut inputs_iter = self.inner.input.iter().peekable();
    let has_name = inputs_iter.peek().is_some_and(|input| input.name.is_some());
    if has_name {
      Either::B(
        self
          .inner
          .input
          .iter()
          .map(|input| {
            (
              input.name.clone().unwrap_or_else(|| {
                unreachable!("Inputs passed from js side are either all have names or not")
              }),
              input.import.clone(),
            )
          })
          .collect(),
      )
    } else {
      Either::A(self.inner.input.iter().map(|input| input.import.clone()).collect())
    }
  }

  #[napi(getter)]
  pub fn cwd(&self) -> Option<String> {
    Some(self.inner.cwd.to_string_lossy().to_string())
  }

  #[napi(getter, ts_return_type = "'node' | 'browser' | 'neutral'")]
  pub fn platform(&self) -> &'static str {
    match &self.inner.platform {
      rolldown::Platform::Node => "node",
      rolldown::Platform::Browser => "browser",
      rolldown::Platform::Neutral => "neutral",
    }
  }

  #[napi(getter)]
  pub fn shim_missing_exports(&self) -> bool {
    self.inner.shim_missing_exports
  }

  #[napi(getter)]
  pub fn name(&self) -> Option<&str> {
    self.inner.name.as_deref()
  }

  // Some options can be set to `None`, and these values are converted to `null` in JavaScript.
  // To distinguish them from regular None values, `undefined` is used to represent unsupported functions
  #[napi(getter)]
  pub fn css_entry_filenames(&self) -> Either<&str, Undefined> {
    match &self.inner.css_entry_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn css_chunk_filenames(&self) -> Either<&str, Undefined> {
    match &self.inner.css_chunk_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn entry_filenames(&self) -> Either<&str, Undefined> {
    match &self.inner.entry_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn chunk_filenames(&self) -> Either<&str, Undefined> {
    match &self.inner.chunk_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn asset_filenames(&self) -> Either<&str, Undefined> {
    match &self.inner.asset_filenames {
      rolldown::AssetFilenamesOutputOption::String(inner) => Either::A(inner),
      rolldown::AssetFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn dir(&self) -> Option<&str> {
    self.inner.dir.as_deref()
  }

  #[napi(getter)]
  pub fn file(&self) -> Option<&str> {
    self.inner.file.as_deref()
  }

  #[napi(getter, ts_return_type = "'es' | 'cjs' | 'iife' | 'umd'")]
  pub fn format(&self) -> &'static str {
    match self.inner.format {
      rolldown::OutputFormat::Esm => "es",
      rolldown::OutputFormat::Cjs => "cjs",
      rolldown::OutputFormat::Iife => "iife",
      rolldown::OutputFormat::Umd => "umd",
    }
  }

  #[napi(getter, ts_return_type = "'default' | 'named' | 'none' | 'auto'")]
  pub fn exports(&self) -> &'static str {
    match self.inner.exports {
      rolldown::OutputExports::Default => "default",
      rolldown::OutputExports::Named => "named",
      rolldown::OutputExports::None => "none",
      rolldown::OutputExports::Auto => "auto",
    }
  }

  #[napi(getter, ts_return_type = "boolean | 'if-default-prop'")]
  pub fn es_module(&self) -> Either<bool, &'static str> {
    match self.inner.es_module {
      rolldown::EsModuleFlag::Always => Either::A(true),
      rolldown::EsModuleFlag::Never => Either::A(false),
      rolldown::EsModuleFlag::IfDefaultProp => Either::B("if-default-prop"),
    }
  }

  #[napi(getter)]
  pub fn inline_dynamic_imports(&self) -> bool {
    self.inner.inline_dynamic_imports
  }

  #[napi(getter, ts_return_type = "boolean | 'inline' | 'hidden'")]
  pub fn sourcemap(&self) -> Either<bool, &'static str> {
    match self.inner.sourcemap {
      Some(rolldown::SourceMapType::File) => Either::A(true),
      Some(rolldown::SourceMapType::Hidden) => Either::B("hidden"),
      Some(rolldown::SourceMapType::Inline) => Either::B("inline"),
      None => Either::A(false),
    }
  }

  #[napi(getter)]
  pub fn sourcemap_base_url(&self) -> Option<&str> {
    self.inner.sourcemap_base_url.as_deref()
  }

  #[napi(getter)]
  pub fn banner(&self) -> Either<Option<&str>, Undefined> {
    match &self.inner.banner {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    }
  }

  #[napi(getter)]
  pub fn footer(&self) -> Either<Option<&str>, Undefined> {
    match &self.inner.footer {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    }
  }

  #[napi(getter)]
  pub fn intro(&self) -> Either<Option<&str>, Undefined> {
    match &self.inner.intro {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    }
  }

  #[napi(getter)]
  pub fn outro(&self) -> Either<Option<&str>, Undefined> {
    match &self.inner.outro {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    }
  }

  #[napi(getter)]
  pub fn external_live_bindings(&self) -> bool {
    self.inner.external_live_bindings
  }

  #[napi(getter)]
  pub fn extend(&self) -> bool {
    self.inner.extend
  }

  #[napi(getter)]
  pub fn globals(&self) -> Either<HashMap<String, String, FxBuildHasher>, Undefined> {
    match &self.inner.globals {
      rolldown::GlobalsOutputOption::FxHashMap(globals) => Either::A(globals.clone()),
      rolldown::GlobalsOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter, ts_return_type = "'base64' | 'base36' | 'hex'")]
  pub fn hash_characters(&self) -> &'static str {
    match self.inner.hash_characters {
      rolldown::HashCharacters::Base64 => "base64",
      rolldown::HashCharacters::Base36 => "base36",
      rolldown::HashCharacters::Hex => "hex",
    }
  }

  #[napi(getter)]
  pub fn sourcemap_debug_ids(&self) -> bool {
    self.inner.sourcemap_debug_ids
  }

  #[napi(getter)]
  pub fn polyfill_require(&self) -> bool {
    self.inner.polyfill_require
  }

  #[napi(getter, ts_return_type = "false | 'dce-only' | MinifyOptions")]
  pub fn minify(&self) -> Either3<bool, &'static str, oxc_minify_napi::MinifyOptions> {
    match &self.inner.minify {
      MinifyOptions::Disabled => Either3::A(false),
      MinifyOptions::DeadCodeEliminationOnly => Either3::B("dce-only"),
      MinifyOptions::Enabled((minify_options, remove_whitespace)) => {
        Either3::C(oxc_minify_napi::MinifyOptions {
          compress: minify_options
            .compress
            .as_ref()
            .map(|compress| Either::B(compress_options_to_napi_compress_options(compress))),
          mangle: minify_options
            .mangle
            .as_ref()
            .map(|mangle| Either::B(mangle_options_to_napi_mangle_options(mangle))),
          codegen: Some(Either::B(codegen_options_to_napi_codegen_options(*remove_whitespace))),
          ..Default::default()
        })
      }
    }
  }

  #[napi(getter, ts_return_type = "'none' | 'inline'")]
  pub fn legal_comments(&self) -> &'static str {
    match self.inner.legal_comments {
      rolldown::LegalComments::None => "none",
      rolldown::LegalComments::Inline => "inline",
    }
  }

  #[napi(getter)]
  pub fn preserve_modules(&self) -> bool {
    self.inner.preserve_modules
  }

  #[napi(getter, ts_return_type = "string | undefined")]
  pub fn preserve_modules_root(&self) -> Option<&str> {
    self.inner.preserve_modules_root.as_deref()
  }

  #[napi(getter)]
  pub fn virtual_dirname(&self) -> &str {
    &self.inner.virtual_dirname
  }

  #[napi(getter)]
  pub fn top_level_var(&self) -> bool {
    self.inner.top_level_var
  }

  #[napi(getter)]
  pub fn minify_internal_exports(&self) -> bool {
    self.inner.minify_internal_exports
  }

  #[napi(getter)]
  pub fn context(&self) -> &str {
    // https://github.com/rolldown/rolldown/issues/5671
    if self.inner.context.is_empty() {
      return "void 0";
    }

    &self.inner.context
  }
}
