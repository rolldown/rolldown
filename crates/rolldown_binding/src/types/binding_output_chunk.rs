use std::collections::HashMap;

use napi_derive::napi;
use rolldown_sourcemap::SourceMap;

use crate::{type_aliases::WeakRefMutex, types::binding_rendered_module::BindingRenderedModule};

#[napi]
pub struct BindingOutputChunk {
  inner: WeakRefMutex<Vec<rolldown_common::Output>>,
  index: usize,
}

#[napi]
impl BindingOutputChunk {
  pub fn new(inner: WeakRefMutex<Vec<rolldown_common::Output>>, index: usize) -> Self {
    Self { inner, index }
  }

  #[napi(getter)]
  pub fn is_entry(&self) -> bool {
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk.is_entry,
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(getter)]
  pub fn is_dynamic_entry(&self) -> bool {
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk.is_dynamic_entry,
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(getter)]
  pub fn facade_module_id(&self) -> Option<String> {
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => {
          chunk.facade_module_id.as_ref().map(|x| x.to_string())
        }
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(getter)]
  pub fn module_ids(&self) -> Vec<String> {
    // self.inner.module_ids.iter().map(|x| x.to_string()).collect()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => {
          chunk.module_ids.iter().map(|x| x.to_string()).collect()
        }
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(getter)]
  pub fn exports(&self) -> Vec<String> {
    // self.inner.exports.clone()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk.exports.clone(),
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  // RenderedChunk
  #[napi(getter)]
  pub fn file_name(&self) -> String {
    // self.inner.filename.to_string()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk.filename.to_string(),
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(getter)]
  pub fn modules(&self) -> HashMap<String, BindingRenderedModule> {
    // self
    //   .inner
    //   .modules
    //   .clone()
    //   .into_iter()
    //   .map(|(key, value)| (key.to_string(), value.into()))
    //   .collect()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk
          .modules
          .clone()
          .into_iter()
          .map(|(key, value)| (key.to_string(), value.into()))
          .collect(),
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(getter)]
  pub fn imports(&self) -> Vec<String> {
    // self.inner.imports.iter().map(|x| x.to_string()).collect()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => {
          chunk.imports.iter().map(|x| x.to_string()).collect()
        }
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(setter, js_name = "imports")]
  pub fn set_imports(&mut self, imports: Vec<String>) {
    // self.inner.imports = imports.into_iter().map(Into::into).collect();
    self.inner.try_with_inner(|inner| {
      let mut inner = inner.lock().unwrap();
      match &mut inner[self.index] {
        rolldown_common::Output::Chunk(ref mut chunk) => {
          chunk.imports = imports.into_iter().map(Into::into).collect();
        }
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    });
  }

  #[napi(getter)]
  pub fn dynamic_imports(&self) -> Vec<String> {
    // self.inner.dynamic_imports.iter().map(|x| x.to_string()).collect()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => {
          chunk.dynamic_imports.iter().map(|x| x.to_string()).collect()
        }
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  // OutputChunk
  #[napi(getter)]
  pub fn code(&self) -> String {
    // self.inner.code.clone()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk.code.clone(),
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(setter, js_name = "code")]
  pub fn set_code(&mut self, code: String) {
    // self.inner.code = code;
    self.inner.try_with_inner(|inner| {
      let mut inner = inner.lock().unwrap();
      match &mut inner[self.index] {
        rolldown_common::Output::Chunk(ref mut chunk) => {
          chunk.code = code;
        }
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    });
  }

  #[napi(getter)]
  pub fn map(&self) -> napi::Result<Option<String>> {
    // Ok(self.inner.map.as_ref().map(SourceMap::to_json_string))
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => {
          Ok(chunk.map.as_ref().map(SourceMap::to_json_string))
        }
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(setter, js_name = "map")]
  pub fn set_map(&mut self, map: String) -> napi::Result<()> {
    // self.inner.map = Some(
    //   SourceMap::from_json_string(map.as_str())
    //     .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?,
    // );
    self.inner.with_inner(|inner| -> napi::Result<()> {
      let mut inner = inner.lock().unwrap();
      match &mut inner[self.index] {
        rolldown_common::Output::Chunk(ref mut chunk) => {
          chunk.map = Some(
            SourceMap::from_json_string(map.as_str())
              .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?,
          );
          Ok(())
        }
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })?;
    Ok(())
  }

  #[napi(getter)]
  pub fn sourcemap_file_name(&self) -> Option<String> {
    // self.inner.sourcemap_filename.clone()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk.sourcemap_filename.clone(),
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(getter)]
  pub fn preliminary_file_name(&self) -> String {
    // self.inner.preliminary_filename.to_string()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk.preliminary_filename.to_string(),
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }

  #[napi(getter)]
  pub fn name(&self) -> String {
    // self.inner.name.to_string()
    self.inner.with_inner(|inner| {
      let inner = inner.lock().unwrap();
      match &inner[self.index] {
        rolldown_common::Output::Chunk(ref chunk) => chunk.name.to_string(),
        rolldown_common::Output::Asset(_) => unreachable!(),
      }
    })
  }
}
