use crate::{BundlerOptions, SharedOptions, SharedResolver};
use rolldown_common::SharedFileEmitter;
use rolldown_error::BuildDiagnostic;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use std::any::Any;

use crate::types::scan_stage_cache::ScanStageCache;

pub struct Bundler {
  pub closed: bool,
  pub(crate) fs: OsFileSystem,
  pub(crate) options: SharedOptions,
  pub(crate) resolver: SharedResolver,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) plugin_driver: SharedPluginDriver,
  pub(crate) warnings: Vec<BuildDiagnostic>,
  pub(crate) _log_guard: Option<Box<dyn Any + Send>>,
  pub(crate) cache: ScanStageCache,
  pub(crate) session: rolldown_debug::Session,
  pub(crate) build_count: u32,
}

fn _test_bundler() {
  fn assert_send(_foo: impl Send) {}
  let mut bundler = Bundler::new(BundlerOptions::default()).expect("Failed to create bundler");
  let write_fut = bundler.write();
  assert_send(write_fut);
  let mut bundler = Bundler::new(BundlerOptions::default()).expect("Failed to create bundler");
  let generate_fut = bundler.generate();
  assert_send(generate_fut);
}
