mod asset;
mod ast_scanner;
mod bundler;
mod css;
pub mod dev;
mod ecmascript;
mod hmr;
mod module_finalizers;
mod module_loader;
mod stages;
mod type_alias;
mod types;
mod utils;
mod watch;
use std::sync::Arc;

use rolldown_fs::OsFileSystem;
use rolldown_resolver::Resolver;

pub(crate) type SharedResolver = Arc<Resolver<OsFileSystem>>;
pub(crate) type SharedOptions = SharedNormalizedBundlerOptions;

pub use crate::{
  bundler::{Bundler, BundlerBuilder},
  dev::dev_engine::DevEngine,
  types::bundle_output::BundleOutput,
  watch::{
    event::{BundleEvent, WatcherEvent},
    Watcher,
  },
};

pub use rolldown_common::bundler_options::*;

pub use rolldown_resolver::ResolveOptions;

pub use rolldown_plugin as plugin;

#[cfg(feature = "testing")]
pub use crate::utils::determine_minify_internal_exports_default::determine_minify_internal_exports_default;
