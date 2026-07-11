#![cfg(not(target_family = "wasm"))]

#[cfg(not(target_family = "wasm"))]
use std::{
  borrow::Cow,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
  time::{Duration, Instant},
};

#[cfg(not(target_family = "wasm"))]
use futures::future::{self, BoxFuture};
#[cfg(not(target_family = "wasm"))]
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin::HookUsage;
#[cfg(not(target_family = "wasm"))]
use rolldown_plugin::Plugin;

use crate::worker_manager::{WorkerManager, WorkerSemaphorePermit};

#[cfg(not(target_family = "wasm"))]
use super::BindingPluginOptions;
use super::JsPlugin;

#[derive(Debug)]
#[cfg_attr(target_family = "wasm", allow(unused))]
pub struct ParallelJsPlugin {
  plugins: Box<[JsPlugin]>,
  worker_manager: Arc<WorkerManager>,
  transform_metrics: Option<ParallelTransformMetrics>,
}

#[cfg(not(target_family = "wasm"))]
impl ParallelJsPlugin {
  pub fn new_boxed(
    plugins: Vec<BindingPluginOptions>,
    worker_manager: Arc<WorkerManager>,
  ) -> napi::Result<Box<dyn Pluginable>> {
    let plugins =
      plugins.into_iter().map(JsPlugin::new).collect::<napi::Result<Vec<_>>>()?.into_boxed_slice();
    let transform_metrics = ParallelTransformMetrics::new_if_enabled(&plugins);
    Ok(Box::new(Self { plugins, worker_manager, transform_metrics }))
  }

  pub fn new_shared(
    plugins: Vec<BindingPluginOptions>,
    worker_manager: Arc<WorkerManager>,
  ) -> napi::Result<Arc<dyn Pluginable>> {
    let plugins =
      plugins.into_iter().map(JsPlugin::new).collect::<napi::Result<Vec<_>>>()?.into_boxed_slice();
    let transform_metrics = ParallelTransformMetrics::new_if_enabled(&plugins);
    Ok(Arc::new(Self { plugins, worker_manager, transform_metrics }))
  }

  fn first_plugin(&self) -> &JsPlugin {
    &self.plugins[0]
  }

  #[cfg(not(target_family = "wasm"))]
  async fn run_single<'a, R, F: FnOnce(&'a JsPlugin) -> BoxFuture<'a, R>>(&'a self, f: F) -> R {
    let permit = self.worker_manager.acquire().await;
    let plugin = &self.plugins[permit.worker_index() as usize];
    f(plugin).await
  }

  #[cfg(not(target_family = "wasm"))]
  async fn run_all<
    'a,
    R,
    E: std::fmt::Debug,
    F: FnMut(&'a JsPlugin) -> BoxFuture<'a, Result<R, E>>,
  >(
    &'a self,
    f: F,
  ) -> Result<Vec<R>, E> {
    let _permit = self.worker_manager.acquire_all().await;
    let results = future::join_all(self.plugins.iter().map(f)).await;
    let mut ok_list: Vec<R> = Vec::with_capacity(results.len());
    for result in results {
      ok_list.push(result?);
    }
    Ok(ok_list)
  }
}

#[cfg(not(target_family = "wasm"))]
impl Plugin for ParallelJsPlugin {
  fn name(&self) -> Cow<'static, str> {
    self.first_plugin().call_name()
  }

  // --- Build hooks ---

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().build_start.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::build_start(plugin, ctx, args))).await?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if self.first_plugin().resolve_id.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::resolve_id(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  async fn load(
    &self,
    ctx: rolldown_plugin::SharedLoadPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if self.first_plugin().load.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::load(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if self.first_plugin().transform.is_some() {
      if let Some(metrics) = &self.transform_metrics {
        let queued_call = QueuedTransformCall::new(metrics, args.code.len());
        let permit = self.worker_manager.acquire().await;
        let permit_acquired_at = Instant::now();
        let active_call = queued_call.acquired(permit, permit_acquired_at);
        let plugin = &self.plugins[active_call.worker_index() as usize];
        let result = Plugin::transform(plugin, ctx, args).await;
        active_call.finish(&result);
        result
      } else {
        self.run_single(|plugin| Box::pin(Plugin::transform(plugin, ctx, args))).await
      }
    } else {
      Ok(None)
    }
  }

  async fn module_parsed(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    module_info: Arc<rolldown_common::ModuleInfo>,
    normal_module: &rolldown_common::NormalModule,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().module_parsed.is_some() {
      self
        .run_all(|plugin| {
          Box::pin(Plugin::module_parsed(plugin, ctx, Arc::clone(&module_info), normal_module))
        })
        .await?;
    }
    Ok(())
  }

  async fn build_end(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().build_end.is_some() {
      self.run_all(|plugin| Box::pin(Plugin::build_end(plugin, ctx, args))).await?;
    }
    Ok(())
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if self.first_plugin().render_chunk.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::render_chunk(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }

  // --- Output hooks ---

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().generate_bundle.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::generate_bundle(plugin, ctx, args))).await
    } else {
      Ok(())
    }
  }

  async fn write_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookWriteBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.first_plugin().write_bundle.is_some() {
      self.run_single(|plugin| Box::pin(Plugin::write_bundle(plugin, ctx, args))).await
    } else {
      Ok(())
    }
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::all()
  }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for ParallelJsPlugin {
  fn drop(&mut self) {
    if let Some(metrics) = &self.transform_metrics {
      metrics.report();
    }
  }
}

#[derive(Debug)]
#[cfg(not(target_family = "wasm"))]
struct ParallelTransformMetrics {
  plugin_name: String,
  worker_count: usize,
  calls: AtomicU64,
  acquired_calls: AtomicU64,
  completed_calls: AtomicU64,
  queue_wait_ns_total: AtomicU64,
  queue_wait_ns_max: AtomicU64,
  service_ns_total: AtomicU64,
  service_ns_max: AtomicU64,
  pending_current: AtomicU64,
  pending_max: AtomicU64,
  outstanding_current: AtomicU64,
  outstanding_max: AtomicU64,
  in_flight_current: AtomicU64,
  in_flight_max: AtomicU64,
  input_code_bytes: AtomicU64,
  returned_code_bytes: AtomicU64,
  value_results: AtomicU64,
  null_results: AtomicU64,
  error_results: AtomicU64,
  cancelled_before_acquire: AtomicU64,
  cancelled_during_service: AtomicU64,
}

#[cfg(not(target_family = "wasm"))]
impl ParallelTransformMetrics {
  fn new_if_enabled(plugins: &[JsPlugin]) -> Option<Self> {
    if std::env::var("ROLLDOWN_PARALLEL_PLUGIN_METRICS").as_deref() != Ok("json") {
      return None;
    }

    Some(Self {
      plugin_name: plugins.first().map_or_else(String::new, |plugin| plugin.name.clone()),
      worker_count: plugins.len(),
      calls: AtomicU64::new(0),
      acquired_calls: AtomicU64::new(0),
      completed_calls: AtomicU64::new(0),
      queue_wait_ns_total: AtomicU64::new(0),
      queue_wait_ns_max: AtomicU64::new(0),
      service_ns_total: AtomicU64::new(0),
      service_ns_max: AtomicU64::new(0),
      pending_current: AtomicU64::new(0),
      pending_max: AtomicU64::new(0),
      outstanding_current: AtomicU64::new(0),
      outstanding_max: AtomicU64::new(0),
      in_flight_current: AtomicU64::new(0),
      in_flight_max: AtomicU64::new(0),
      input_code_bytes: AtomicU64::new(0),
      returned_code_bytes: AtomicU64::new(0),
      value_results: AtomicU64::new(0),
      null_results: AtomicU64::new(0),
      error_results: AtomicU64::new(0),
      cancelled_before_acquire: AtomicU64::new(0),
      cancelled_during_service: AtomicU64::new(0),
    })
  }

  fn report(&self) {
    let load = |value: &AtomicU64| value.load(Ordering::Relaxed);
    let report = serde_json::json!({
      "kind": "rolldown_parallel_plugin_transform_metrics",
      "version": 1,
      "plugin": self.plugin_name,
      "workerCount": self.worker_count,
      "wrapperCalls": load(&self.calls),
      "permitAcquiredCalls": load(&self.acquired_calls),
      "completedWrapperCalls": load(&self.completed_calls),
      "permitQueueWaitNs": {
        "total": load(&self.queue_wait_ns_total),
        "max": load(&self.queue_wait_ns_max),
      },
      "permitHeldNs": {
        "total": load(&self.service_ns_total),
        "max": load(&self.service_ns_max),
      },
      "permitQueuePending": {
        "current": load(&self.pending_current),
        "max": load(&self.pending_max),
      },
      "wrapperOutstanding": {
        "current": load(&self.outstanding_current),
        "max": load(&self.outstanding_max),
      },
      "permitInFlight": {
        "current": load(&self.in_flight_current),
        "max": load(&self.in_flight_max),
      },
      "wrapperInputCodeBytes": load(&self.input_code_bytes),
      "returnedCodeBytes": load(&self.returned_code_bytes),
      "valueResults": load(&self.value_results),
      "nullResults": load(&self.null_results),
      "errorResults": load(&self.error_results),
      "cancelledBeforeAcquire": load(&self.cancelled_before_acquire),
      "cancelledDuringService": load(&self.cancelled_during_service),
    });
    eprintln!("[rolldown-parallel-plugin-metrics] {report}");
  }
}

#[cfg(not(target_family = "wasm"))]
struct QueuedTransformCall<'a> {
  metrics: &'a ParallelTransformMetrics,
  started_at: Instant,
  acquired: bool,
}

#[cfg(not(target_family = "wasm"))]
impl<'a> QueuedTransformCall<'a> {
  fn new(metrics: &'a ParallelTransformMetrics, input_code_bytes: usize) -> Self {
    metrics.calls.fetch_add(1, Ordering::Relaxed);
    metrics.input_code_bytes.fetch_add(input_code_bytes as u64, Ordering::Relaxed);
    let pending = metrics.pending_current.fetch_add(1, Ordering::Relaxed) + 1;
    metrics.pending_max.fetch_max(pending, Ordering::Relaxed);
    let outstanding = metrics.outstanding_current.fetch_add(1, Ordering::Relaxed) + 1;
    metrics.outstanding_max.fetch_max(outstanding, Ordering::Relaxed);
    Self { metrics, started_at: Instant::now(), acquired: false }
  }

  fn acquired(
    mut self,
    permit: WorkerSemaphorePermit,
    permit_acquired_at: Instant,
  ) -> ActiveTransformCall<'a> {
    let queue_wait_ns = duration_ns(permit_acquired_at.duration_since(self.started_at));
    self.metrics.queue_wait_ns_total.fetch_add(queue_wait_ns, Ordering::Relaxed);
    self.metrics.queue_wait_ns_max.fetch_max(queue_wait_ns, Ordering::Relaxed);
    self.metrics.pending_current.fetch_sub(1, Ordering::Relaxed);
    self.metrics.acquired_calls.fetch_add(1, Ordering::Relaxed);
    self.acquired = true;
    ActiveTransformCall::new(self.metrics, permit, permit_acquired_at)
  }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for QueuedTransformCall<'_> {
  fn drop(&mut self) {
    if !self.acquired {
      self.metrics.pending_current.fetch_sub(1, Ordering::Relaxed);
      self.metrics.outstanding_current.fetch_sub(1, Ordering::Relaxed);
      self.metrics.cancelled_before_acquire.fetch_add(1, Ordering::Relaxed);
    }
  }
}

#[cfg(not(target_family = "wasm"))]
struct ActiveTransformCall<'a> {
  metrics: &'a ParallelTransformMetrics,
  permit: Option<WorkerSemaphorePermit>,
  started_at: Instant,
  completed: bool,
}

#[cfg(not(target_family = "wasm"))]
impl<'a> ActiveTransformCall<'a> {
  fn new(
    metrics: &'a ParallelTransformMetrics,
    permit: WorkerSemaphorePermit,
    started_at: Instant,
  ) -> Self {
    let in_flight = metrics.in_flight_current.fetch_add(1, Ordering::Relaxed) + 1;
    metrics.in_flight_max.fetch_max(in_flight, Ordering::Relaxed);
    Self { metrics, permit: Some(permit), started_at, completed: false }
  }

  fn worker_index(&self) -> u16 {
    self.permit.as_ref().expect("active transform call must own a worker permit").worker_index()
  }

  fn finish(mut self, result: &rolldown_plugin::HookTransformReturn) {
    self.release_permit();
    self.completed = true;
    self.metrics.completed_calls.fetch_add(1, Ordering::Relaxed);
    match result {
      Ok(Some(output)) => {
        self.metrics.value_results.fetch_add(1, Ordering::Relaxed);
        if let Some(code) = &output.code {
          self.metrics.returned_code_bytes.fetch_add(code.len() as u64, Ordering::Relaxed);
        }
      }
      Ok(None) => {
        self.metrics.null_results.fetch_add(1, Ordering::Relaxed);
      }
      Err(_) => {
        self.metrics.error_results.fetch_add(1, Ordering::Relaxed);
      }
    }
  }

  fn release_permit(&mut self) {
    drop(self.permit.take());
    let service_ns = duration_ns(self.started_at.elapsed());
    self.metrics.service_ns_total.fetch_add(service_ns, Ordering::Relaxed);
    self.metrics.service_ns_max.fetch_max(service_ns, Ordering::Relaxed);
    self.metrics.in_flight_current.fetch_sub(1, Ordering::Relaxed);
    self.metrics.outstanding_current.fetch_sub(1, Ordering::Relaxed);
  }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for ActiveTransformCall<'_> {
  fn drop(&mut self) {
    if self.permit.is_some() {
      self.release_permit();
    }
    if !self.completed {
      self.metrics.cancelled_during_service.fetch_add(1, Ordering::Relaxed);
    }
  }
}

#[cfg(not(target_family = "wasm"))]
fn duration_ns(duration: Duration) -> u64 {
  duration.as_nanos().try_into().unwrap_or(u64::MAX)
}
