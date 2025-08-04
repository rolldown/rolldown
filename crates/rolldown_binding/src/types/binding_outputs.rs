use std::sync::Arc;

use super::{
  binding_output_asset::{BindingOutputAsset, JsOutputAsset},
  binding_output_chunk::{BindingOutputChunk, JsOutputChunk, update_output_chunk},
};
use napi_derive::napi;
use rolldown_error::{BuildDiagnostic, DiagnosticOptions};

// The `BindingOutputs` take the data to js side, the rust side will not use it anymore.
#[napi]
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
    Self { chunks, assets, error: None }
  }
}

#[napi(object)]

pub struct JsChangedOutputs {
  pub chunks: Vec<JsOutputChunk>,
  pub assets: Vec<JsOutputAsset>,
  pub deleted: Vec<String>,
}

pub fn update_outputs(
  outputs: &mut Vec<rolldown_common::Output>,
  changed: JsChangedOutputs,
) -> anyhow::Result<()> {
  for chunk in changed.chunks {
    if let Some(index) = outputs.iter().position(|o| o.filename() == chunk.filename) {
      match &mut outputs[index] {
        rolldown_common::Output::Chunk(old_chunk) => {
          update_output_chunk(old_chunk, chunk)?;
        }
        rolldown_common::Output::Asset(_) => {}
      }
    }
  }
  for asset in changed.assets {
    if let Some(index) = outputs.iter().position(|o| o.filename() == asset.filename) {
      outputs[index] = rolldown_common::Output::Asset(Arc::new(asset.into()));
    }
  }
  for deleted in changed.deleted {
    if let Some(index) = outputs.iter().position(|o| o.filename() == deleted) {
      outputs.remove(index);
    }
  }
  Ok(())
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
      let e = napi::JsError::from(napi_error.try_clone().unwrap_or_else(|e| e));
      napi::Either::A(e)
    }
    Err(error) => napi::Either::B(BindingError {
      kind: error.kind().to_string(),
      message: error.to_diagnostic_with(&DiagnosticOptions { cwd }).to_color_string(),
    }),
  }
}
