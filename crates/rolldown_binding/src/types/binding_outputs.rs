use super::{
  binding_output_asset::{BindingOutputAsset, JsOutputAsset},
  binding_output_chunk::{BindingOutputChunk, JsOutputChunk},
};
use napi::Env;
use napi_derive::napi;
use rolldown_error::{BuildDiagnostic, DiagnosticOptions};

// The `BindingOutputs` take the data to js side, the rust side will not use it anymore.
#[napi]
pub struct BindingOutputs {
  chunks: Vec<BindingOutputChunk>,
  assets: Vec<BindingOutputAsset>,
  // TODO: BindingBuildDiagnostic?
  errors: Vec<BuildDiagnostic>,
}

#[napi]
impl BindingOutputs {
  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    std::mem::take(&mut self.chunks)
  }

  #[napi(getter)]
  pub fn assets(&mut self) -> Vec<BindingOutputAsset> {
    std::mem::take(&mut self.assets)
  }

  #[napi(getter)]
  pub fn errors(&mut self, env: Env) -> napi::Result<Option<napi::JsObject>> {
    if self.errors.is_empty() {
      return Ok(None);
    }
    let mut objects_array = env.create_array_with_length(self.errors.len())?;
    for (i, error) in std::mem::take(&mut self.errors).into_iter().enumerate() {
      let js_error = match error.downcast_napi_error() {
        Ok(napi_error) => napi::JsError::from(napi_error).into_unknown(env),
        Err(error) => {
          let mut object = env.create_object()?;
          object.set("kind", error.kind().to_string())?;
          object.set(
            "message",
            format!(
              "{}",
              error
                // TODO: cwd
                // .into_diagnostic_with(&DiagnosticOptions { cwd: self.cwd.clone() })
                .into_diagnostic_with(&DiagnosticOptions::default())
                .to_color_string()
            ),
          )?;
          object.into_unknown()
        }
      };
      objects_array.set_element(i as u32, js_error)?;
    }
    Ok(Some(objects_array))
  }
}

impl From<Vec<rolldown_common::Output>> for BindingOutputs {
  fn from(outputs: Vec<rolldown_common::Output>) -> Self {
    let mut chunks = vec![];
    let mut assets = vec![];
    outputs.into_iter().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => {
        chunks.push(BindingOutputChunk::new(*chunk));
      }
      rolldown_common::Output::Asset(asset) => {
        assets.push(BindingOutputAsset::new(*asset));
      }
    });
    Self { chunks, assets, errors: vec![] }
  }
}

impl From<Vec<BuildDiagnostic>> for BindingOutputs {
  fn from(errors: Vec<BuildDiagnostic>) -> Self {
    Self { chunks: vec![], assets: vec![], errors }
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
      outputs[index] = rolldown_common::Output::Chunk(Box::new(chunk.try_into()?));
    }
  }
  for asset in changed.assets {
    if let Some(index) = outputs.iter().position(|o| o.filename() == asset.filename) {
      outputs[index] = rolldown_common::Output::Asset(Box::new(asset.into()));
    }
  }
  for deleted in changed.deleted {
    if let Some(index) = outputs.iter().position(|o| o.filename() == deleted) {
      outputs.remove(index);
    }
  }
  Ok(())
}
