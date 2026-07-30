mod ast_scanner;
mod bundle;
mod bundler;
mod bundler_builder;
mod chunk_graph;
mod ecmascript;
mod esm_init_obligations;
mod hmr;
mod module_finalizers;
mod module_loader;
mod stages;
mod type_alias;
mod types;
mod utils;
use std::sync::Arc;

use rolldown_resolver::Resolver;

pub(crate) type SharedResolver<Fs> = Arc<Resolver<Fs>>;
pub(crate) type SharedOptions = SharedNormalizedBundlerOptions;

pub use crate::{
  bundle::{
    bundle::Bundle,
    bundle_factory::{BundleFactory, BundleFactoryOptions},
    bundle_handle::BundleHandle,
  },
  bundler::Bundler,
  bundler_builder::BundlerBuilder,
  types::{bundle_output::BundleOutput, bundler_config::BundlerConfig},
};

pub use rolldown_common::bundler_options::*;

pub use rolldown_resolver::ResolveOptions;

pub use rolldown_plugin as plugin;

/// Wait for heavy build-state values queued for background destruction.
///
/// This is an internal integration point for the native binding, which must ensure all deferred
/// frees finish before asking the process allocator to return unused pages to the operating system.
#[doc(hidden)]
pub fn drain_deferred_drops() {
  utils::defer_drop::drain();
}

#[cfg(feature = "testing")]
pub use crate::utils::determine_minify_internal_exports_default;
