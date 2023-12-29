use derivative::Derivative;
use napi_derive::napi;
use rolldown_fs::OsFileSystem;
use serde::Deserialize;

#[derive(Debug)]
#[napi]
pub struct PluginContext {
  inner: &'static rolldown::PluginContext<OsFileSystem>,
}

#[napi]
impl PluginContext {
  #[napi]
  pub fn load(&self) {
    self.inner.load();
  }

  #[napi]
  pub async fn resolve(&self, source: String, importer: Option<String>) -> napi::Result<ResolveId> {
    let result = self.inner.resolve(source, importer).await;

    match result {
      Ok(value) => Ok(value.into()),
      Err(err) => {
        // TODO: better handing errors
        eprintln!("{err:?}");
        Err(napi::Error::from_reason("Build failed"))
      }
    }
  }
}

impl<'a> From<&'a rolldown::PluginContext<OsFileSystem>> for PluginContext {
  fn from(inner: &'a rolldown::PluginContext<OsFileSystem>) -> Self {
    unsafe { Self { inner: std::mem::transmute(inner) } }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct ResolveId {
  pub external: bool,
  pub id: String,
}

impl From<rolldown::ResolveId> for ResolveId {
  fn from(value: rolldown::ResolveId) -> Self {
    Self { external: value.external, id: value.id }
  }
}

#[derive(Debug)]
#[napi]
pub struct TransformPluginContext {
  inner: &'static rolldown::TransformPluginContext<OsFileSystem, 'static>,
}

#[napi]
impl TransformPluginContext {
  #[napi]
  pub fn get_ctx(&self) -> PluginContext {
    self.inner.inner.into()
  }
}

impl<'a> From<&'a rolldown::TransformPluginContext<'_, OsFileSystem>> for TransformPluginContext {
  fn from(inner: &'a rolldown::TransformPluginContext<OsFileSystem>) -> Self {
    unsafe { Self { inner: std::mem::transmute(inner) } }
  }
}
