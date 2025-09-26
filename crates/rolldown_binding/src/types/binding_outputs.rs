use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use super::{
  binding_output_asset::{BindingOutputAsset, JsOutputAsset},
  binding_output_chunk::{BindingOutputChunk, JsOutputChunk, update_output_chunk},
};
use napi::Either;
use napi_derive::napi;
use rolldown_error::{BuildDiagnostic, DiagnosticOptions};
use rustc_hash::FxBuildHasher;

// The `BindingOutputs` take the data to js side, the rust side will not use it anymore.
#[napi]
#[derive(Default)]
pub struct BindingOutputs {
  chunks: Vec<BindingOutputChunk>,
  assets: Vec<BindingOutputAsset>,
  error: Option<rolldown_common::OutputsDiagnostics>,
}

#[napi]
impl BindingOutputs {
  pub(crate) fn chunk_len(&self) -> usize {
    self.chunks.len()
  }

  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    std::mem::take(&mut self.chunks)
  }

  #[napi(getter)]
  pub fn assets(&mut self) -> Vec<BindingOutputAsset> {
    std::mem::take(&mut self.assets)
  }

  #[napi(getter)]
  pub fn errors(&mut self) -> Vec<napi::Either<napi::JsError, BindingError>> {
    if let Some(rolldown_common::OutputsDiagnostics { diagnostics, cwd }) = self.error.as_ref() {
      return diagnostics
        .iter()
        .map(|diagnostic| to_js_diagnostic(diagnostic, cwd.clone()))
        .collect();
    }
    vec![]
  }

  pub fn from_errors(diagnostics: Vec<BuildDiagnostic>, cwd: std::path::PathBuf) -> Self {
    let error = rolldown_common::OutputsDiagnostics { diagnostics, cwd };
    Self { assets: vec![], chunks: vec![], error: Some(error) }
  }
}

impl From<&Vec<rolldown_common::Output>> for BindingOutputs {
  fn from(outputs: &Vec<rolldown_common::Output>) -> Self {
    let mut chunks = vec![];
    let mut assets = vec![];
    outputs.iter().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => {
        chunks.push(BindingOutputChunk::new(Arc::downgrade(&chunk)));
      }
      rolldown_common::Output::Asset(asset) => {
        assets.push(BindingOutputAsset::new(Arc::downgrade(&asset)));
      }
    });
    Self { chunks, assets, error: None }
  }
}

#[napi(object)]
pub struct JsChangedOutputs {
  pub deleted: HashSet<String, FxBuildHasher>,
  pub changes: HashMap<String, Either<JsOutputChunk, JsOutputAsset>, FxBuildHasher>,
}

impl JsChangedOutputs {
  pub fn apply_changes(
    &mut self,
    outputs: &mut Vec<rolldown_common::Output>,
  ) -> anyhow::Result<()> {
    let mut result = Ok(());
    if !self.deleted.is_empty() || !self.changes.is_empty() {
      outputs.retain_mut(|output| {
        if result.is_err() {
          return true;
        }
        let filename = output.filename();
        if self.deleted.contains(filename) {
          return false;
        }
        if let Some(change) = self.changes.remove(filename) {
          match (output, change) {
            (rolldown_common::Output::Chunk(old_chunk), Either::A(chunk)) => {
              if let Err(err) = update_output_chunk(old_chunk, chunk) {
                result = Err(err);
              }
            }
            (v @ rolldown_common::Output::Asset(_), Either::B(asset)) => {
              *v = rolldown_common::Output::Asset(Arc::new(asset.into()));
            }
            _ => {}
          }
        }
        true
      });
    }
    result
  }
}

#[napi(object)]
pub struct BindingError {
  pub kind: String,
  pub message: String,
}

pub fn to_js_diagnostic(
  diagnostic: &BuildDiagnostic,
  cwd: std::path::PathBuf,
) -> napi::Either<napi::JsError, BindingError> {
  match diagnostic.downcast_napi_error() {
    Ok(napi_error) => {
      // Note: In WASM workers, napi::Error objects with maybe_raw/maybe_env references cannot be
      // safely shared across threads, which would cause try_clone() to fail. Currently, we don't
      // guarantee full JS error consistency in WASM environments. In the future, we could enhance
      // the BindingError fields to preserve all custom error properties and achieve complete JS
      // error consistency across all environments.
      #[cfg(not(target_family = "wasm"))]
      {
        let error = napi_error.try_clone().unwrap_or_else(|e| e);
        napi::Either::A(napi::JsError::from(error))
      }
      #[cfg(target_family = "wasm")]
      {
        let error = napi::Error::new(napi_error.status, napi_error.reason.clone());
        napi::Either::A(napi::JsError::from(error))
      }
    }
    Err(error) => napi::Either::B(BindingError {
      kind: error.kind().to_string(),
      message: error.to_diagnostic_with(&DiagnosticOptions { cwd }).to_color_string(),
    }),
  }
}
