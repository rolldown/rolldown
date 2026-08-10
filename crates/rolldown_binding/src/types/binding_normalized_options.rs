use std::collections::HashMap;
use std::sync::Arc;

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

use super::external_memory_status::ExternalMemoryStatus;

#[napi]
pub struct BindingNormalizedOptions {
  inner: Option<SharedNormalizedBundlerOptions>,
}

#[napi]
impl BindingNormalizedOptions {
  pub fn new(inner: SharedNormalizedBundlerOptions) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&SharedNormalizedBundlerOptions> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed: this normalized-options box's native data was eagerly released after its hook invocation settled. Copy the fields you need during the hook.",
      )
    })
  }

  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self) -> ExternalMemoryStatus {
    match self.inner.take() {
      None => ExternalMemoryStatus {
        freed: false,
        reason: Some("Memory has already been freed".to_string()),
      },
      Some(arc) => {
        let strong_count = Arc::strong_count(&arc);
        if strong_count > 1 {
          ExternalMemoryStatus {
            freed: false,
            reason: Some(format!(
              "Data has been dropped, but there are {} other strong reference(s) referring to this data on the native side, so the memory may not be released.",
              strong_count - 1
            )),
          }
        } else {
          ExternalMemoryStatus { freed: true, reason: None }
        }
      }
    }
  }

  // Notice: rust's HashMap doesn't guarantee the order of keys, so not sure if it's a good idea to expose it to JS directly.
  #[napi(getter)]
  pub fn input(&self) -> napi::Result<Either<Vec<String>, HashMap<String, String, FxBuildHasher>>> {
    let inner = self.try_get_inner()?;
    let mut inputs_iter = inner.input.iter().peekable();
    let has_name = inputs_iter.peek().is_some_and(|input| input.name.is_some());
    Ok(if has_name {
      Either::B(
        inner
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
      Either::A(inner.input.iter().map(|input| input.import.clone()).collect())
    })
  }

  #[napi(getter)]
  pub fn cwd(&self) -> napi::Result<String> {
    Ok(self.try_get_inner()?.cwd.to_string_lossy().to_string())
  }

  #[napi(getter, ts_return_type = "'node' | 'browser' | 'neutral'")]
  pub fn platform(&self) -> napi::Result<&'static str> {
    Ok(match &self.try_get_inner()?.platform {
      rolldown::Platform::Node => "node",
      rolldown::Platform::Browser => "browser",
      rolldown::Platform::Neutral => "neutral",
    })
  }

  #[napi(getter)]
  pub fn shim_missing_exports(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.shim_missing_exports)
  }

  #[napi(getter)]
  pub fn name(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.name.as_deref())
  }

  #[napi(getter)]
  pub fn entry_filenames(&self) -> napi::Result<Either<&str, Undefined>> {
    Ok(match &self.try_get_inner()?.entry_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    })
  }

  #[napi(getter)]
  pub fn chunk_filenames(&self) -> napi::Result<Either<&str, Undefined>> {
    Ok(match &self.try_get_inner()?.chunk_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    })
  }

  #[napi(getter)]
  pub fn sourcemap_filenames(&self) -> napi::Result<Either<&str, Undefined>> {
    Ok(match &self.try_get_inner()?.sourcemap_filenames {
      Some(rolldown::ChunkFilenamesOutputOption::String(inner)) => Either::A(inner),
      Some(rolldown::ChunkFilenamesOutputOption::Fn(_)) | None => Either::B(()),
    })
  }

  #[napi(getter)]
  pub fn asset_filenames(&self) -> napi::Result<Either<&str, Undefined>> {
    Ok(match &self.try_get_inner()?.asset_filenames {
      rolldown::AssetFilenamesOutputOption::String(inner) => Either::A(inner),
      rolldown::AssetFilenamesOutputOption::Fn(_) => Either::B(()),
    })
  }

  #[napi(getter)]
  pub fn dir(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.dir.as_deref())
  }

  #[napi(getter)]
  pub fn file(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.file.as_deref())
  }

  #[napi(getter, ts_return_type = "'es' | 'cjs' | 'iife' | 'umd'")]
  pub fn format(&self) -> napi::Result<&'static str> {
    Ok(self.try_get_inner()?.format.as_str())
  }

  #[napi(getter, ts_return_type = "'default' | 'named' | 'none' | 'auto'")]
  pub fn exports(&self) -> napi::Result<&'static str> {
    Ok(match self.try_get_inner()?.exports {
      rolldown::OutputExports::Default => "default",
      rolldown::OutputExports::Named => "named",
      rolldown::OutputExports::None => "none",
      rolldown::OutputExports::Auto => "auto",
    })
  }

  #[napi(getter, ts_return_type = "boolean | 'if-default-prop'")]
  pub fn es_module(&self) -> napi::Result<Either<bool, &'static str>> {
    Ok(match self.try_get_inner()?.es_module {
      rolldown::EsModuleFlag::Always => Either::A(true),
      rolldown::EsModuleFlag::Never => Either::A(false),
      rolldown::EsModuleFlag::IfDefaultProp => Either::B("if-default-prop"),
    })
  }

  #[napi(getter)]
  pub fn code_splitting(&self) -> napi::Result<bool> {
    // The normalized layer never holds the `Advanced` object form (it is decomposed
    // into the gate + `manual_code_splitting` during normalization), but match it
    // exhaustively as "enabled" for completeness.
    Ok(match &self.try_get_inner()?.code_splitting {
      rolldown_common::CodeSplittingMode::Bool(v) => *v,
      rolldown_common::CodeSplittingMode::Advanced(_) => true,
    })
  }

  #[napi(getter)]
  pub fn dynamic_import_in_cjs(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.dynamic_import_in_cjs)
  }

  #[napi(getter, ts_return_type = "boolean | 'inline' | 'hidden'")]
  pub fn sourcemap(&self) -> napi::Result<Either<bool, &'static str>> {
    Ok(match self.try_get_inner()?.sourcemap {
      Some(rolldown::SourceMapType::File) => Either::A(true),
      Some(rolldown::SourceMapType::Hidden) => Either::B("hidden"),
      Some(rolldown::SourceMapType::Inline) => Either::B("inline"),
      None => Either::A(false),
    })
  }

  #[napi(getter)]
  pub fn sourcemap_base_url(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.sourcemap_base_url.as_deref())
  }

  #[napi(getter)]
  pub fn banner(&self) -> napi::Result<Either<Option<&str>, Undefined>> {
    Ok(match &self.try_get_inner()?.banner {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    })
  }

  #[napi(getter)]
  pub fn footer(&self) -> napi::Result<Either<Option<&str>, Undefined>> {
    Ok(match &self.try_get_inner()?.footer {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    })
  }

  #[napi(getter)]
  pub fn intro(&self) -> napi::Result<Either<Option<&str>, Undefined>> {
    Ok(match &self.try_get_inner()?.intro {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    })
  }

  #[napi(getter)]
  pub fn outro(&self) -> napi::Result<Either<Option<&str>, Undefined>> {
    Ok(match &self.try_get_inner()?.outro {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    })
  }

  #[napi(getter)]
  pub fn post_banner(&self) -> napi::Result<Either<Option<&str>, Undefined>> {
    Ok(match &self.try_get_inner()?.post_banner {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    })
  }

  #[napi(getter)]
  pub fn post_footer(&self) -> napi::Result<Either<Option<&str>, Undefined>> {
    Ok(match &self.try_get_inner()?.post_footer {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.as_deref()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    })
  }

  #[napi(getter)]
  pub fn external_live_bindings(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.external_live_bindings)
  }

  #[napi(getter)]
  pub fn extend(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.extend)
  }

  #[napi(getter)]
  pub fn globals(&self) -> napi::Result<Either<HashMap<String, String, FxBuildHasher>, Undefined>> {
    Ok(match &self.try_get_inner()?.globals {
      rolldown::GlobalsOutputOption::FxHashMap(globals) => Either::A(globals.clone()),
      rolldown::GlobalsOutputOption::Fn(_) => Either::B(()),
    })
  }

  #[napi(getter, ts_return_type = "'base64' | 'base36' | 'hex'")]
  pub fn hash_characters(&self) -> napi::Result<&'static str> {
    Ok(match self.try_get_inner()?.hash_characters {
      rolldown::HashCharacters::Base64 => "base64",
      rolldown::HashCharacters::Base36 => "base36",
      rolldown::HashCharacters::Hex => "hex",
    })
  }

  #[napi(getter)]
  pub fn sourcemap_debug_ids(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.sourcemap_debug_ids)
  }

  #[napi(getter)]
  pub fn sourcemap_exclude_sources(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.sourcemap_exclude_sources)
  }

  #[napi(getter)]
  pub fn polyfill_require(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.polyfill_require)
  }

  #[napi(getter, ts_return_type = "false | 'dce-only' | MinifyOptions")]
  pub fn minify(
    &self,
  ) -> napi::Result<Either3<bool, &'static str, oxc_minify_napi::MinifyOptions>> {
    Ok(match &self.try_get_inner()?.minify {
      MinifyOptions::Disabled => Either3::A(false),
      MinifyOptions::DeadCodeEliminationOnly(_) => Either3::B("dce-only"),
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
    })
  }

  #[napi(getter, ts_return_type = "'none' | 'inline'")]
  pub fn legal_comments(&self) -> napi::Result<&'static str> {
    Ok(match self.try_get_inner()?.legal_comments {
      rolldown::LegalComments::None => "none",
      rolldown::LegalComments::Inline => "inline",
    })
  }

  #[napi(getter)]
  pub fn comments(&self) -> napi::Result<crate::options::BindingCommentsOptions> {
    let inner = self.try_get_inner()?;
    Ok(crate::options::BindingCommentsOptions {
      legal: Some(inner.comments.legal),
      annotation: Some(inner.comments.annotation),
      jsdoc: Some(inner.comments.jsdoc),
    })
  }

  #[napi(getter)]
  pub fn preserve_modules(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.preserve_modules)
  }

  #[napi(getter, ts_return_type = "string | undefined")]
  pub fn preserve_modules_root(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.preserve_modules_root.as_deref())
  }

  #[napi(getter)]
  pub fn virtual_dirname(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.virtual_dirname)
  }

  #[napi(getter)]
  pub fn top_level_var(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.top_level_var)
  }

  #[napi(getter)]
  pub fn minify_internal_exports(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.minify_internal_exports)
  }

  #[napi(getter)]
  pub fn context(&self) -> napi::Result<&str> {
    let inner = self.try_get_inner()?;
    // https://github.com/rolldown/rolldown/issues/5671
    if inner.context.is_empty() {
      return Ok("void 0");
    }

    Ok(&inner.context)
  }
}
