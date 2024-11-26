use std::collections::HashMap;

use napi::{bindgen_prelude::Undefined, Either};
use napi_derive::napi;
use rolldown::SharedNormalizedBundlerOptions;

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
  pub fn input(&self) -> Either<Vec<String>, HashMap<String, String>> {
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
  pub fn platform(&self) -> String {
    match &self.inner.platform {
      rolldown::Platform::Node => "node".to_string(),
      rolldown::Platform::Browser => "browser".to_string(),
      rolldown::Platform::Neutral => "neutral".to_string(),
    }
  }

  #[napi(getter)]
  pub fn shim_missing_exports(&self) -> bool {
    self.inner.shim_missing_exports
  }

  #[napi(getter)]
  pub fn name(&self) -> Option<String> {
    self.inner.name.clone()
  }

  // Some options can be set to `None`, and these values are converted to `null` in JavaScript.
  // To distinguish them from regular None values, `undefined` is used to represent unsupported functions
  #[napi(getter)]
  pub fn css_entry_filenames(&self) -> Either<String, Undefined> {
    match &self.inner.css_entry_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner.clone()),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn css_chunk_filenames(&self) -> Either<String, Undefined> {
    match &self.inner.css_chunk_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner.clone()),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn entry_filenames(&self) -> Either<String, Undefined> {
    match &self.inner.entry_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner.clone()),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn chunk_filenames(&self) -> Either<String, Undefined> {
    match &self.inner.chunk_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Either::A(inner.clone()),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => Either::B(()),
    }
  }

  #[napi(getter)]
  pub fn asset_filenames(&self) -> String {
    self.inner.asset_filenames.template().to_string()
  }

  #[napi(getter)]
  pub fn dir(&self) -> Option<String> {
    // NOTE: rollup returns undefined when `dir` is not set
    Some(self.inner.dir.clone())
  }

  #[napi(getter)]
  pub fn file(&self) -> Option<String> {
    self.inner.file.clone()
  }

  #[napi(getter, ts_return_type = "'es' | 'cjs' | 'app' | 'iife' | 'umd'")]
  pub fn format(&self) -> String {
    match self.inner.format {
      rolldown::OutputFormat::Esm => "es".to_string(),
      rolldown::OutputFormat::Cjs => "cjs".to_string(),
      rolldown::OutputFormat::App => "app".to_string(),
      rolldown::OutputFormat::Iife => "iife".to_string(),
      rolldown::OutputFormat::Umd => "umd".to_string(),
    }
  }

  #[napi(getter, ts_return_type = "'default' | 'named' | 'none' | 'auto'")]
  pub fn exports(&self) -> String {
    match self.inner.exports {
      rolldown::OutputExports::Default => "default".to_string(),
      rolldown::OutputExports::Named => "named".to_string(),
      rolldown::OutputExports::None => "none".to_string(),
      rolldown::OutputExports::Auto => "auto".to_string(),
    }
  }

  #[napi(getter, ts_return_type = "boolean | 'if-default-prop'")]
  pub fn es_module(&self) -> Either<bool, String> {
    match self.inner.es_module {
      rolldown::EsModuleFlag::Always => Either::A(true),
      rolldown::EsModuleFlag::Never => Either::A(false),
      rolldown::EsModuleFlag::IfDefaultProp => Either::B("if-default-prop".to_string()),
    }
  }

  #[napi(getter)]
  pub fn inline_dynamic_imports(&self) -> bool {
    self.inner.inline_dynamic_imports
  }

  #[napi(getter, ts_return_type = "boolean | 'inline' | 'hidden'")]
  pub fn sourcemap(&self) -> Either<bool, String> {
    match self.inner.sourcemap {
      Some(rolldown::SourceMapType::File) => Either::A(true),
      Some(rolldown::SourceMapType::Hidden) => Either::B("hidden".to_string()),
      Some(rolldown::SourceMapType::Inline) => Either::B("inline".to_string()),
      None => Either::A(false),
    }
  }

  #[napi(getter)]
  pub fn banner(&self) -> Either<Option<String>, Undefined> {
    match &self.inner.banner {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.clone()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    }
  }

  #[napi(getter)]
  pub fn footer(&self) -> Either<Option<String>, Undefined> {
    match &self.inner.footer {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.clone()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    }
  }

  #[napi(getter)]
  pub fn intro(&self) -> Either<Option<String>, Undefined> {
    match &self.inner.intro {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.clone()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    }
  }

  #[napi(getter)]
  pub fn outro(&self) -> Either<Option<String>, Undefined> {
    match &self.inner.outro {
      Some(rolldown::AddonOutputOption::String(inner)) => Either::A(inner.clone()),
      Some(rolldown::AddonOutputOption::Fn(_)) => Either::B(()),
      None => Either::A(None),
    }
  }

  #[napi(getter)]
  pub fn external_live_bindings(&self) -> bool {
    self.inner.external_live_bindings
  }
}
