use std::{
  path::PathBuf,
  sync::{Arc, RwLock, atomic::AtomicU32},
  time::Instant,
};

use rolldown_plugin_vite_reporter::{LogInfoFn, ViteReporterPlugin};
use sugar_path::SugarPath as _;

use crate::types::js_callback::{JsCallback, JsCallbackExt as _};

#[napi_derive::napi(object, object_to_js = false)]
#[expect(clippy::struct_excessive_bools)]
pub struct BindingViteReporterPluginConfig {
  pub root: String,
  pub is_tty: bool,
  pub is_lib: bool,
  pub assets_dir: String,
  pub chunk_limit: f64,
  pub warn_large_chunks: bool,
  pub report_compressed_size: bool,
  #[napi(ts_type = "(msg: string) => void")]
  pub log_info: Option<JsCallback<String>>,
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
impl From<BindingViteReporterPluginConfig> for ViteReporterPlugin {
  fn from(config: BindingViteReporterPluginConfig) -> Self {
    Self {
      root: PathBuf::from(config.root).normalize(),
      is_lib: config.is_lib,
      is_tty: config.is_tty,
      assets_dir: config.assets_dir,
      chunk_limit: config.chunk_limit as usize,
      warn_large_chunks: config.warn_large_chunks,
      report_compressed_size: config.report_compressed_size,
      chunk_count: AtomicU32::new(0),
      transformed_count: AtomicU32::new(0),
      latest_checkpoint: Arc::new(RwLock::new(Instant::now())),
      log_info: config.log_info.map(|log_info| -> Arc<LogInfoFn> {
        Arc::new(move |msg: String| {
          let cb = Arc::clone(&log_info);
          Box::pin(async move { cb.invoke_async(msg).await.map_err(anyhow::Error::from) })
        })
      }),
    }
  }
}
