use std::{
  path::PathBuf,
  sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicU32},
  },
  time::Instant,
};

use rolldown_plugin_vite_reporter::ViteReporterPlugin;
use sugar_path::SugarPath as _;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
#[expect(clippy::struct_excessive_bools)]
pub struct BindingViteReporterPluginConfig {
  pub root: String,
  pub is_tty: bool,
  pub is_lib: bool,
  pub assets_dir: String,
  pub chunk_limit: f64,
  pub should_log_info: bool,
  pub warn_large_chunks: bool,
  pub report_compressed_size: bool,
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
      should_log_info: config.should_log_info,
      warn_large_chunks: config.warn_large_chunks,
      report_compressed_size: config.report_compressed_size,
      chunk_count: AtomicU32::new(0),
      compressed_count: AtomicU32::new(0),
      has_rendered_chunk: AtomicBool::new(false),
      has_transformed: AtomicBool::new(false),
      transformed_count: AtomicU32::new(0),
      latest_checkpoint: Arc::new(RwLock::new(Instant::now())),
    }
  }
}
