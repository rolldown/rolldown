use rolldown::BundlerConfig;

use crate::{
  types::binding_bundler_options::BindingBundlerOptions,
  utils::normalize_binding_options::normalize_binding_options,
};

#[cfg(not(target_family = "wasm"))]
use crate::utils::normalize_binding_options::NativePluginMaterializationMetrics;
#[cfg(not(target_family = "wasm"))]
use std::time::Instant;

pub fn create_bundler_config_from_binding_options(
  option: BindingBundlerOptions,
) -> napi::Result<BundlerConfig> {
  let BindingBundlerOptions {
    input_options,
    output_options,
    parallel_plugins_registry,
    metrics_id,
  } = option;

  #[cfg(not(target_family = "wasm"))]
  let metrics_enabled = metrics_id.is_some()
    && std::env::var("ROLLDOWN_PARALLEL_PLUGIN_METRICS").as_deref() == Ok("json");
  #[cfg(not(target_family = "wasm"))]
  let native_normalization_started_at = metrics_enabled.then(Instant::now);

  #[cfg(not(target_family = "wasm"))]
  let registry_transfer_started_at = metrics_enabled.then(Instant::now);
  #[cfg(not(target_family = "wasm"))]
  let worker_count =
    parallel_plugins_registry.as_ref().map(|registry| registry.worker_count).unwrap_or_default();
  #[cfg(not(target_family = "wasm"))]
  let parallel_registry_present = parallel_plugins_registry.is_some();
  #[cfg(not(target_family = "wasm"))]
  let parallel_plugins_map =
    parallel_plugins_registry.map(|registry| registry.take_plugin_values());
  #[cfg(not(target_family = "wasm"))]
  let registry_transfer_ms =
    registry_transfer_started_at.map(|started_at| started_at.elapsed().as_secs_f64() * 1_000.0);

  #[cfg(not(target_family = "wasm"))]
  let worker_manager_construction_started_at = metrics_enabled.then(Instant::now);
  #[cfg(not(target_family = "wasm"))]
  let worker_manager = if worker_count > 0 {
    use crate::worker_manager::WorkerManager;
    Some(WorkerManager::new(worker_count))
  } else {
    None
  };
  #[cfg(not(target_family = "wasm"))]
  let worker_manager_construction_ms = worker_manager_construction_started_at
    .map(|started_at| started_at.elapsed().as_secs_f64() * 1_000.0);

  #[cfg(not(target_family = "wasm"))]
  let mut plugin_metrics = metrics_enabled.then(NativePluginMaterializationMetrics::new);
  #[cfg(not(target_family = "wasm"))]
  let binding_option_normalization_started_at = metrics_enabled.then(Instant::now);
  let config = normalize_binding_options(
    input_options,
    output_options,
    #[cfg(not(target_family = "wasm"))]
    parallel_plugins_map,
    #[cfg(not(target_family = "wasm"))]
    worker_manager,
    #[cfg(not(target_family = "wasm"))]
    plugin_metrics.as_mut(),
  )?;
  #[cfg(not(target_family = "wasm"))]
  let binding_option_normalization_ms = binding_option_normalization_started_at
    .map(|started_at| started_at.elapsed().as_secs_f64() * 1_000.0);

  #[cfg(not(target_family = "wasm"))]
  if let (Some(metrics_id), Some(plugin_metrics)) = (metrics_id, plugin_metrics) {
    let ordinary_js_plugin_count =
      plugin_metrics.plugins.iter().filter(|record| record["kind"] == "ordinary-js").count();
    let parallel_js_plugin_count =
      plugin_metrics.plugins.iter().filter(|record| record["kind"] == "parallel-js").count();
    let builtin_plugin_count =
      plugin_metrics.plugins.iter().filter(|record| record["kind"] == "builtin").count();
    let native_normalization_total_ms = native_normalization_started_at
      .expect("metrics timer exists when metrics are enabled")
      .elapsed()
      .as_secs_f64()
      * 1_000.0;
    let report = serde_json::json!({
      "kind": "rolldown_native_plugin_registration_metrics",
      "version": 1,
      "metricsId": metrics_id,
      "boundary": "after BindingBundlerOptions destructuring, before registry transfer, through BundlerConfig construction, synchronously before ClassicBundler::create_bundle and Bundle::scan",
      "nativeNormalizationTotalMs": native_normalization_total_ms,
      "nativePluginMaterializationMs": plugin_metrics.duration_ms,
      "stages": {
        "registryTransferMs": registry_transfer_ms.expect("metrics timer exists"),
        "workerManagerConstructionMs": worker_manager_construction_ms.expect("metrics timer exists"),
        "bindingOptionNormalizationMs": binding_option_normalization_ms.expect("metrics timer exists"),
        "pluginMaterializationMs": plugin_metrics.duration_ms,
      },
      "stageRelationships": {
        "registryTransfer": "direct child of nativeNormalizationTotal",
        "workerManagerConstruction": "direct child of nativeNormalizationTotal",
        "bindingOptionNormalization": "direct child of nativeNormalizationTotal",
        "pluginMaterialization": "nested inside bindingOptionNormalization",
      },
      "parallelRegistryPresent": parallel_registry_present,
      "workerManagerWorkerCount": worker_count,
      "ordinaryJsPluginCount": ordinary_js_plugin_count,
      "parallelJsPluginCount": parallel_js_plugin_count,
      "builtinPluginCount": builtin_plugin_count,
      "plugins": plugin_metrics.plugins,
      "scope": "The total includes registry transfer, WorkerManager construction, all binding-option normalization, plugin conversion, and BundlerConfig construction. It excludes JavaScript bindingification, create_bundle, scan, hooks, and build time.",
    });
    eprintln!("[rolldown-native-plugin-registration-metrics] {report}");
  }

  #[cfg(target_family = "wasm")]
  let _ = metrics_id;

  Ok(config)
}
