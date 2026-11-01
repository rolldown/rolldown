mod ast_scanner;
mod bundle;
mod bundler;
mod bundler_builder;
mod chunk_graph;
mod deferred_resolution_error;
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

pub use deferred_resolution_error::DeferredResolutionError;
pub use rolldown_common::bundler_options::*;
pub use utils::native_cell_plan;
pub use utils::native_record_ast;
pub use utils::native_records::{NATIVE_JSON_NAMESPACE_PREFIX, NATIVE_JSON_RECORD_PREFIX};
pub use utils::topology_config;

pub use rolldown_resolver::ResolveOptions;

pub use crate::types::scan_stage_cache::{
  CodegenCacheBudget, CodegenCacheTrim, CommonjsInterop, NativeGraphChange, RevisionScanCache,
};

pub use rolldown_plugin as plugin;

#[cfg(feature = "testing")]
pub use crate::utils::determine_minify_internal_exports_default;
