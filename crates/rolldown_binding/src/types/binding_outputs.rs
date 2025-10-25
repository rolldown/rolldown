use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use super::{
  binding_output_asset::{BindingOutputAsset, JsOutputAsset},
  binding_output_chunk::{BindingOutputChunk, JsOutputChunk, update_output_chunk},
  error::BindingError,
};
use napi::Either;
use napi_derive::napi;
use rolldown_error::{BuildDiagnostic, DiagnosticOptions};
use rustc_hash::FxBuildHasher;

#[napi(object, object_from_js = false)]
pub struct BindingOutputs {
  pub chunks: Vec<BindingOutputChunk>,
  pub assets: Vec<BindingOutputAsset>,
}

impl From<Vec<rolldown_common::Output>> for BindingOutputs {
  fn from(outputs: Vec<rolldown_common::Output>) -> Self {
    let mut chunks = vec![];
    let mut assets = vec![];
    outputs.into_iter().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => {
        chunks.push(BindingOutputChunk::new(chunk));
      }
      rolldown_common::Output::Asset(asset) => {
        assets.push(BindingOutputAsset::new(asset));
      }
    });
    Self { chunks, assets }
  }
}

#[napi_derive::napi(object, object_to_js = false)]
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

pub fn to_binding_error(diagnostic: &BuildDiagnostic, cwd: std::path::PathBuf) -> BindingError {
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
        BindingError::JsError(napi::JsError::from(error))
      }
      #[cfg(target_family = "wasm")]
      {
        let error = napi::Error::new(napi_error.status, napi_error.reason.clone());
        BindingError::JsError(napi::JsError::from(error))
      }
    }
    Err(error) => BindingError::NativeError(super::error::native_error::NativeError {
      kind: error.kind().to_string(),
      message: error.to_diagnostic_with(&DiagnosticOptions { cwd }).to_color_string(),
    }),
  }
}
