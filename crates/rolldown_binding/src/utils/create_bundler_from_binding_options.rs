use rolldown::{Bundler, BundlerBuilder};

use crate::{
  types::binding_bundler_options::BindingBundlerOptions,
  utils::normalize_binding_options::normalize_binding_options,
};

pub fn create_bundler_from_binding_options(option: BindingBundlerOptions) -> napi::Result<Bundler> {
  let BindingBundlerOptions { input_options, output_options, parallel_plugins_registry } = option;

  #[cfg(not(target_family = "wasm"))]
  let worker_count =
    parallel_plugins_registry.as_ref().map(|registry| registry.worker_count).unwrap_or_default();
  #[cfg(not(target_family = "wasm"))]
  let parallel_plugins_map =
    parallel_plugins_registry.map(|registry| registry.take_plugin_values());

  #[cfg(not(target_family = "wasm"))]
  let worker_manager = if worker_count > 0 {
    use crate::worker_manager::WorkerManager;
    Some(WorkerManager::new(worker_count))
  } else {
    None
  };

  let ret = normalize_binding_options(
    input_options,
    output_options,
    #[cfg(not(target_family = "wasm"))]
    parallel_plugins_map,
    #[cfg(not(target_family = "wasm"))]
    worker_manager,
  )?;

  let bundler_builder =
    BundlerBuilder::default().with_options(ret.bundler_options).with_plugins(ret.plugins);

  // TODO: improve the following error message
  let bundler = bundler_builder.build().map_err(|err| {
    napi::Error::new(
      napi::Status::GenericFailure,
      err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
    )
  })?;

  Ok(bundler)
}
