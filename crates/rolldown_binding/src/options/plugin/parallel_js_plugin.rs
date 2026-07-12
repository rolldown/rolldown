#![cfg(not(target_family = "wasm"))]

#[cfg(not(target_family = "wasm"))]
use std::{
  borrow::Cow,
  sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
  },
  time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
        let queued_call = QueuedTransformCall::new(metrics, args.id, args.code.len());
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
  epoch: Instant,
  epoch_unix_ns: u64,
  epoch_alignment_uncertainty_ns: u64,
  timeline: Mutex<TransformTimeline>,
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

    let (epoch, epoch_unix_ns, epoch_alignment_uncertainty_ns) = clock_epoch();
    Some(Self {
      plugin_name: plugins.first().map_or_else(String::new, |plugin| plugin.name.clone()),
      worker_count: plugins.len(),
      epoch,
      epoch_unix_ns,
      epoch_alignment_uncertainty_ns,
      timeline: Mutex::new(TransformTimeline::new(plugins.len())),
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
    let report_anchor = clock_anchor(self.epoch);
    let mut timeline = self
      .timeline
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .report(report_anchor.relative_ns);
    timeline["clock"] = serde_json::json!({
      "relativeUnit": "nanoseconds",
      "zeroUnixEpochNs": self.epoch_unix_ns.to_string(),
      "zeroAlignmentUncertaintyNs": self.epoch_alignment_uncertainty_ns,
      "reportedRelativeNs": report_anchor.relative_ns,
      "reportedUnixEpochNs": report_anchor.unix_ns.to_string(),
      "reportAlignmentUncertaintyNs": report_anchor.uncertainty_ns,
      "interpretation": "Each event atNs is relative to zeroUnixEpochNs. Alignment uses bracketed Instant/SystemTime samples; uncertainty excludes later system-clock adjustment."
    });
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
      "timeline": timeline,
    });
    eprintln!("[rolldown-parallel-plugin-metrics] {report}");
  }

  fn record_timeline_arrival(&self, module_id: &str) -> u64 {
    let mut timeline = self.timeline.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let at_ns = duration_ns(self.epoch.elapsed());
    timeline.record_arrival(at_ns, module_id)
  }

  fn record_timeline_event(
    &self,
    call_id: u64,
    phase: TransformEventPhase,
    worker_index: Option<u16>,
    service_ns: Option<u64>,
  ) {
    let mut timeline = self.timeline.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let at_ns = duration_ns(self.epoch.elapsed());
    timeline.record(at_ns, call_id, phase, worker_index, service_ns);
  }
}

#[derive(Debug)]
#[cfg(not(target_family = "wasm"))]
struct ClockAnchor {
  relative_ns: u64,
  unix_ns: u64,
  uncertainty_ns: u64,
}

#[cfg(not(target_family = "wasm"))]
fn clock_epoch() -> (Instant, u64, u64) {
  let before = Instant::now();
  let unix_ns = system_time_ns();
  let after = Instant::now();
  let bracket = after.duration_since(before);
  (before + bracket / 2, unix_ns, duration_ns(bracket))
}

#[cfg(not(target_family = "wasm"))]
fn clock_anchor(epoch: Instant) -> ClockAnchor {
  let before = Instant::now();
  let unix_ns = system_time_ns();
  let after = Instant::now();
  let bracket = after.duration_since(before);
  let midpoint = before + bracket / 2;
  ClockAnchor {
    relative_ns: duration_ns(midpoint.saturating_duration_since(epoch)),
    unix_ns,
    uncertainty_ns: duration_ns(bracket),
  }
}

#[cfg(not(target_family = "wasm"))]
fn system_time_ns() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map(duration_ns).unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(not(target_family = "wasm"))]
enum TransformEventPhase {
  Arrival,
  Acquire,
  Complete,
  CancelBeforeAcquire,
  CancelDuringService,
}

#[cfg(not(target_family = "wasm"))]
impl TransformEventPhase {
  fn as_str(self) -> &'static str {
    match self {
      Self::Arrival => "arrival",
      Self::Acquire => "acquire",
      Self::Complete => "complete",
      Self::CancelBeforeAcquire => "cancel_before_acquire",
      Self::CancelDuringService => "cancel_during_service",
    }
  }
}

#[derive(Debug)]
#[cfg(not(target_family = "wasm"))]
struct TransformTimelineEvent {
  sequence: u64,
  call_id: u64,
  phase: TransformEventPhase,
  at_ns: u64,
  worker_index: Option<u16>,
}

#[derive(Debug)]
#[cfg(not(target_family = "wasm"))]
struct TransformTimelineCall {
  ordinal: u64,
  module_id: String,
}

#[derive(Debug)]
#[cfg(not(target_family = "wasm"))]
struct TransformTimeline {
  calls: Vec<TransformTimelineCall>,
  events: Vec<TransformTimelineEvent>,
  first_event_ns: Option<u64>,
  last_event_ns: Option<u64>,
  last_accounted_ns: u64,
  pending: u64,
  outstanding: u64,
  in_flight: u64,
  pending_width_ns: u64,
  outstanding_width_ns: u64,
  in_flight_width_ns: u64,
  completed_at_ns: Vec<u64>,
  worker_completed_service_ns: Vec<Vec<u64>>,
}

#[cfg(not(target_family = "wasm"))]
impl TransformTimeline {
  fn new(worker_count: usize) -> Self {
    Self {
      calls: Vec::new(),
      events: Vec::new(),
      first_event_ns: None,
      last_event_ns: None,
      last_accounted_ns: 0,
      pending: 0,
      outstanding: 0,
      in_flight: 0,
      pending_width_ns: 0,
      outstanding_width_ns: 0,
      in_flight_width_ns: 0,
      completed_at_ns: Vec::new(),
      worker_completed_service_ns: (0..worker_count).map(|_| Vec::new()).collect(),
    }
  }

  fn record_arrival(&mut self, at_ns: u64, module_id: &str) -> u64 {
    let ordinal = self.calls.len() as u64 + 1;
    self.calls.push(TransformTimelineCall { ordinal, module_id: module_id.to_owned() });
    self.record(at_ns, ordinal, TransformEventPhase::Arrival, None, None);
    ordinal
  }

  fn record(
    &mut self,
    at_ns: u64,
    call_id: u64,
    phase: TransformEventPhase,
    worker_index: Option<u16>,
    service_ns: Option<u64>,
  ) {
    let at_ns = at_ns.max(self.last_accounted_ns);
    self.advance_width_areas(at_ns);
    self.last_event_ns = Some(at_ns);
    match phase {
      TransformEventPhase::Arrival => {
        self.pending += 1;
        self.outstanding += 1;
      }
      TransformEventPhase::Acquire => {
        self.pending = self.pending.saturating_sub(1);
        self.in_flight += 1;
      }
      TransformEventPhase::Complete => {
        self.in_flight = self.in_flight.saturating_sub(1);
        self.outstanding = self.outstanding.saturating_sub(1);
        self.completed_at_ns.push(at_ns);
        if let (Some(worker_index), Some(service_ns)) = (worker_index, service_ns)
          && let Some(samples) = self.worker_completed_service_ns.get_mut(worker_index as usize)
        {
          samples.push(service_ns);
        }
      }
      TransformEventPhase::CancelBeforeAcquire => {
        self.pending = self.pending.saturating_sub(1);
        self.outstanding = self.outstanding.saturating_sub(1);
      }
      TransformEventPhase::CancelDuringService => {
        self.in_flight = self.in_flight.saturating_sub(1);
        self.outstanding = self.outstanding.saturating_sub(1);
      }
    }
    self.events.push(TransformTimelineEvent {
      sequence: self.events.len() as u64,
      call_id,
      phase,
      at_ns,
      worker_index,
    });
  }

  fn advance_width_areas(&mut self, at_ns: u64) {
    if self.first_event_ns.is_none() {
      self.first_event_ns = Some(at_ns);
      self.last_accounted_ns = at_ns;
      return;
    }
    let elapsed_ns = at_ns.saturating_sub(self.last_accounted_ns);
    self.pending_width_ns =
      self.pending_width_ns.saturating_add(elapsed_ns.saturating_mul(self.pending));
    self.outstanding_width_ns =
      self.outstanding_width_ns.saturating_add(elapsed_ns.saturating_mul(self.outstanding));
    self.in_flight_width_ns =
      self.in_flight_width_ns.saturating_add(elapsed_ns.saturating_mul(self.in_flight));
    self.last_accounted_ns = at_ns;
  }

  fn report(&mut self, report_at_ns: u64) -> serde_json::Value {
    let report_at_ns = report_at_ns.max(self.last_accounted_ns);
    let activity_end_ns = if self.pending == 0 && self.outstanding == 0 && self.in_flight == 0 {
      self.last_event_ns.unwrap_or(report_at_ns)
    } else {
      self.advance_width_areas(report_at_ns);
      report_at_ns
    };
    let observation_ns =
      self.first_event_ns.map_or(0, |first| activity_end_ns.saturating_sub(first));
    let first_completion_ns = self.completed_at_ns.first().copied();
    let last_completion_ns = self.completed_at_ns.last().copied();
    let completion_span_ns = first_completion_ns
      .zip(last_completion_ns)
      .map_or(0, |(first, last)| last.saturating_sub(first));
    let calls = self
      .calls
      .iter()
      .map(|call| serde_json::json!({ "ordinal": call.ordinal, "moduleId": call.module_id }))
      .collect::<Vec<_>>();
    let events = self
      .events
      .iter()
      .map(|event| {
        serde_json::json!({
          "sequence": event.sequence,
          "callOrdinal": event.call_id,
          "phase": event.phase.as_str(),
          "atNs": event.at_ns,
          "workerIndex": event.worker_index,
        })
      })
      .collect::<Vec<_>>();
    let worker_service = self
      .worker_completed_service_ns
      .iter()
      .enumerate()
      .map(|(worker_index, samples)| worker_service_report(worker_index, samples))
      .collect::<Vec<_>>();
    serde_json::json!({
      "clock": "process monotonic clock; zero is ParallelTransformMetrics construction",
      "reportedAtNs": report_at_ns,
      "activityEndNs": activity_end_ns,
      "calls": calls,
      "events": events,
      "timeWeightedWidths": {
        "observationNs": observation_ns,
        "pendingWidthNs": self.pending_width_ns,
        "outstandingWidthNs": self.outstanding_width_ns,
        "inFlightWidthNs": self.in_flight_width_ns,
      },
      "completionRateInputs": {
        "completedCalls": self.completed_at_ns.len(),
        "activitySpanNs": observation_ns,
        "firstCompletionNs": first_completion_ns,
        "lastCompletionNs": last_completion_ns,
        "completionSpanNs": completion_span_ns,
      },
      "workerServiceNs": worker_service,
    })
  }
}

#[cfg(not(target_family = "wasm"))]
fn worker_service_report(worker_index: usize, samples: &[u64]) -> serde_json::Value {
  let mut sorted = samples.to_vec();
  sorted.sort_unstable();
  let total = sorted.iter().copied().fold(0_u64, u64::saturating_add);
  serde_json::json!({
    "workerIndex": worker_index,
    "completedCalls": sorted.len(),
    "total": total,
    "min": sorted.first(),
    "p50": nearest_rank(&sorted, 50),
    "p95": nearest_rank(&sorted, 95),
    "max": sorted.last(),
  })
}

#[cfg(not(target_family = "wasm"))]
fn nearest_rank(sorted: &[u64], percentile: usize) -> Option<u64> {
  if sorted.is_empty() {
    return None;
  }
  let rank = sorted.len().saturating_mul(percentile).div_ceil(100).max(1);
  sorted.get(rank - 1).copied()
}

#[cfg(not(target_family = "wasm"))]
struct QueuedTransformCall<'a> {
  metrics: &'a ParallelTransformMetrics,
  call_id: u64,
  started_at: Instant,
  acquired: bool,
}

#[cfg(not(target_family = "wasm"))]
impl<'a> QueuedTransformCall<'a> {
  fn new(metrics: &'a ParallelTransformMetrics, module_id: &str, input_code_bytes: usize) -> Self {
    metrics.calls.fetch_add(1, Ordering::Relaxed);
    metrics.input_code_bytes.fetch_add(input_code_bytes as u64, Ordering::Relaxed);
    let pending = metrics.pending_current.fetch_add(1, Ordering::Relaxed) + 1;
    metrics.pending_max.fetch_max(pending, Ordering::Relaxed);
    let outstanding = metrics.outstanding_current.fetch_add(1, Ordering::Relaxed) + 1;
    metrics.outstanding_max.fetch_max(outstanding, Ordering::Relaxed);
    let call_id = metrics.record_timeline_arrival(module_id);
    Self { metrics, call_id, started_at: Instant::now(), acquired: false }
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
    let worker_index = permit.worker_index();
    self.metrics.record_timeline_event(
      self.call_id,
      TransformEventPhase::Acquire,
      Some(worker_index),
      None,
    );
    self.acquired = true;
    ActiveTransformCall::new(self.metrics, self.call_id, permit, permit_acquired_at)
  }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for QueuedTransformCall<'_> {
  fn drop(&mut self) {
    if !self.acquired {
      self.metrics.pending_current.fetch_sub(1, Ordering::Relaxed);
      self.metrics.outstanding_current.fetch_sub(1, Ordering::Relaxed);
      self.metrics.cancelled_before_acquire.fetch_add(1, Ordering::Relaxed);
      self.metrics.record_timeline_event(
        self.call_id,
        TransformEventPhase::CancelBeforeAcquire,
        None,
        None,
      );
    }
  }
}

#[cfg(not(target_family = "wasm"))]
struct ActiveTransformCall<'a> {
  metrics: &'a ParallelTransformMetrics,
  call_id: u64,
  permit: Option<WorkerSemaphorePermit>,
  started_at: Instant,
  completed: bool,
}

#[cfg(not(target_family = "wasm"))]
impl<'a> ActiveTransformCall<'a> {
  fn new(
    metrics: &'a ParallelTransformMetrics,
    call_id: u64,
    permit: WorkerSemaphorePermit,
    started_at: Instant,
  ) -> Self {
    let in_flight = metrics.in_flight_current.fetch_add(1, Ordering::Relaxed) + 1;
    metrics.in_flight_max.fetch_max(in_flight, Ordering::Relaxed);
    Self { metrics, call_id, permit: Some(permit), started_at, completed: false }
  }

  fn worker_index(&self) -> u16 {
    self.permit.as_ref().expect("active transform call must own a worker permit").worker_index()
  }

  fn finish(mut self, result: &rolldown_plugin::HookTransformReturn) {
    self.release_permit(TransformEventPhase::Complete);
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

  fn release_permit(&mut self, phase: TransformEventPhase) {
    let worker_index = self.worker_index();
    let service_ns = duration_ns(self.started_at.elapsed());
    // Remove this call from the logical in-flight count before returning the
    // permit, because a queued call may acquire it immediately during drop.
    self.metrics.in_flight_current.fetch_sub(1, Ordering::Relaxed);
    self.metrics.service_ns_total.fetch_add(service_ns, Ordering::Relaxed);
    self.metrics.service_ns_max.fetch_max(service_ns, Ordering::Relaxed);
    self.metrics.outstanding_current.fetch_sub(1, Ordering::Relaxed);
    self.metrics.record_timeline_event(self.call_id, phase, Some(worker_index), Some(service_ns));
    drop(self.permit.take());
  }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for ActiveTransformCall<'_> {
  fn drop(&mut self) {
    if self.permit.is_some() {
      self.release_permit(TransformEventPhase::CancelDuringService);
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

#[cfg(test)]
mod tests {
  use super::{TransformEventPhase, TransformTimeline, nearest_rank};

  #[test]
  fn transform_timeline_accounts_time_weighted_widths() {
    let mut timeline = TransformTimeline::new(2);
    assert_eq!(timeline.record_arrival(0, "/src/one.js"), 1);
    assert_eq!(timeline.record_arrival(10, "/src/two.js"), 2);
    timeline.record(20, 1, TransformEventPhase::Acquire, Some(0), None);
    timeline.record(50, 1, TransformEventPhase::Complete, Some(0), Some(30));
    timeline.record(60, 2, TransformEventPhase::Acquire, Some(1), None);
    timeline.record(100, 2, TransformEventPhase::Complete, Some(1), Some(40));

    let report = timeline.report(110);
    let widths = &report["timeWeightedWidths"];
    assert_eq!(widths["observationNs"], 100);
    assert_eq!(widths["pendingWidthNs"], 70);
    assert_eq!(widths["outstandingWidthNs"], 140);
    assert_eq!(widths["inFlightWidthNs"], 70);
    assert_eq!(report["completionRateInputs"]["completedCalls"], 2);
    assert_eq!(report["calls"][0]["moduleId"], "/src/one.js");
    assert_eq!(report["calls"][1]["ordinal"], 2);
    assert_eq!(report["workerServiceNs"][0]["p50"], 30);
    assert_eq!(report["workerServiceNs"][1]["p95"], 40);

    let phases = report["events"]
      .as_array()
      .unwrap()
      .iter()
      .map(|event| event["phase"].as_str().unwrap())
      .collect::<Vec<_>>();
    assert_eq!(phases, ["arrival", "arrival", "acquire", "complete", "acquire", "complete"]);
  }

  #[test]
  fn transform_timeline_closes_cancelled_widths() {
    let mut timeline = TransformTimeline::new(1);
    timeline.record_arrival(5, "/src/cancel-before.js");
    timeline.record(15, 1, TransformEventPhase::CancelBeforeAcquire, None, None);
    timeline.record_arrival(20, "/src/cancel-during.js");
    timeline.record(25, 2, TransformEventPhase::Acquire, Some(0), None);
    timeline.record(35, 2, TransformEventPhase::CancelDuringService, Some(0), Some(10));

    let report = timeline.report(45);
    assert_eq!(report["timeWeightedWidths"]["pendingWidthNs"], 15);
    assert_eq!(report["timeWeightedWidths"]["outstandingWidthNs"], 25);
    assert_eq!(report["timeWeightedWidths"]["inFlightWidthNs"], 10);
    assert_eq!(report["completionRateInputs"]["completedCalls"], 0);
  }

  #[test]
  fn nearest_rank_uses_observed_samples() {
    assert_eq!(nearest_rank(&[], 95), None);
    assert_eq!(nearest_rank(&[1, 2, 3, 4], 50), Some(2));
    assert_eq!(nearest_rank(&[1, 2, 3, 4], 95), Some(4));
  }
}
