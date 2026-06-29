#![cfg_attr(all(test, not(feature = "async-runtime")), allow(dead_code))]

#[cfg(feature = "async-runtime")]
use std::{future::Future, pin::Pin};

#[cfg(feature = "async-runtime")]
use napi::bindgen_prelude::{AsyncRuntime, create_custom_async_runtime};
use napi_derive::napi;
#[cfg(feature = "async-runtime")]
use rolldown_utils::async_runtime::{
  RuntimeFlavor, RuntimeMetricsSnapshot, RuntimeOptions, block_on_dyn, configure,
  configured_options, metrics, reset_metrics, shutdown, spawn_detached,
};

#[cfg(feature = "async-runtime")]
struct RolldownAsyncRuntime;

#[cfg(feature = "async-runtime")]
impl AsyncRuntime for RolldownAsyncRuntime {
  fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
    spawn_detached(future);
  }

  fn block_on(&self, future: Pin<&mut dyn Future<Output = ()>>) {
    block_on_dyn(future);
  }

  fn shutdown(&self) {
    shutdown();
  }
}

#[napi(string_enum)]
#[derive(Clone, Copy)]
pub enum BindingRuntimeFlavor {
  CurrentThread,
  MultiThread,
}

#[cfg(feature = "async-runtime")]
impl From<BindingRuntimeFlavor> for RuntimeFlavor {
  fn from(value: BindingRuntimeFlavor) -> Self {
    match value {
      BindingRuntimeFlavor::CurrentThread => Self::CurrentThread,
      BindingRuntimeFlavor::MultiThread => Self::MultiThread,
    }
  }
}

#[cfg(feature = "async-runtime")]
impl From<RuntimeFlavor> for BindingRuntimeFlavor {
  fn from(value: RuntimeFlavor) -> Self {
    match value {
      RuntimeFlavor::CurrentThread => Self::CurrentThread,
      RuntimeFlavor::MultiThread => Self::MultiThread,
    }
  }
}

#[napi(object)]
pub struct BindingRuntimeOptions {
  pub flavor: Option<BindingRuntimeFlavor>,
  pub worker_threads: Option<u32>,
  pub max_blocking_tasks: Option<u32>,
}

#[napi(object)]
pub struct BindingRuntimeConfig {
  pub flavor: BindingRuntimeFlavor,
  pub worker_threads: u32,
  pub max_blocking_tasks: u32,
}

#[cfg(feature = "async-runtime")]
impl From<RuntimeOptions> for BindingRuntimeConfig {
  fn from(value: RuntimeOptions) -> Self {
    Self {
      flavor: value.flavor.into(),
      worker_threads: saturating_u32(value.worker_threads as u64),
      max_blocking_tasks: saturating_u32(value.max_blocking_tasks as u64),
    }
  }
}

#[napi(object)]
pub struct BindingRuntimeMetrics {
  pub flavor: BindingRuntimeFlavor,
  pub worker_threads: u32,
  pub max_blocking_tasks: u32,
  pub tasks_spawned: u32,
  pub tasks_completed: u32,
  pub tasks_panicked: u32,
  pub runnable_schedules: u32,
  pub runnable_polls: u32,
  pub queued_runnables: u32,
  pub max_queued_runnables: u32,
  pub active_runnables: u32,
  pub max_active_runnables: u32,
  pub blocking_tasks_started: u32,
  pub blocking_tasks_completed: u32,
  pub active_blocking_tasks: u32,
  pub max_active_blocking_tasks: u32,
}

#[cfg(feature = "async-runtime")]
impl From<RuntimeMetricsSnapshot> for BindingRuntimeMetrics {
  fn from(value: RuntimeMetricsSnapshot) -> Self {
    Self {
      flavor: value.flavor.into(),
      worker_threads: saturating_u32(value.worker_threads as u64),
      max_blocking_tasks: saturating_u32(value.max_blocking_tasks as u64),
      tasks_spawned: saturating_u32(value.tasks_spawned),
      tasks_completed: saturating_u32(value.tasks_completed),
      tasks_panicked: saturating_u32(value.tasks_panicked),
      runnable_schedules: saturating_u32(value.runnable_schedules),
      runnable_polls: saturating_u32(value.runnable_polls),
      queued_runnables: saturating_u32(value.queued_runnables),
      max_queued_runnables: saturating_u32(value.max_queued_runnables),
      active_runnables: saturating_u32(value.active_runnables),
      max_active_runnables: saturating_u32(value.max_active_runnables),
      blocking_tasks_started: saturating_u32(value.blocking_tasks_started),
      blocking_tasks_completed: saturating_u32(value.blocking_tasks_completed),
      active_blocking_tasks: saturating_u32(value.active_blocking_tasks),
      max_active_blocking_tasks: saturating_u32(value.max_active_blocking_tasks),
    }
  }
}

fn saturating_u32(value: u64) -> u32 {
  u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(feature = "async-runtime")]
#[napi]
pub fn configure_async_runtime(options: BindingRuntimeOptions) -> napi::Result<()> {
  let mut current = configured_options();
  if let Some(flavor) = options.flavor {
    current.flavor = flavor.into();
  }
  if let Some(worker_threads) = options.worker_threads {
    current.worker_threads = worker_threads as usize;
  }
  if let Some(max_blocking_tasks) = options.max_blocking_tasks {
    current.max_blocking_tasks = max_blocking_tasks as usize;
  }
  configure(current).map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
pub fn configure_async_runtime(options: BindingRuntimeOptions) -> napi::Result<()> {
  let _ = options;
  Err(napi::Error::from_reason(
    "This Rolldown binding was built without the `async-runtime` feature",
  ))
}

#[cfg(feature = "async-runtime")]
#[napi]
pub fn get_async_runtime_config() -> BindingRuntimeConfig {
  configured_options().into()
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
pub fn get_async_runtime_config() -> BindingRuntimeConfig {
  let worker_threads = std::env::var("ROLLDOWN_WORKER_THREADS")
    .ok()
    .and_then(|value| value.parse::<usize>().ok())
    .unwrap_or_else(|| num_cpus::get_physical() * 3 / 2);
  let max_blocking_tasks = std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS")
    .ok()
    .and_then(|value| value.parse::<usize>().ok())
    .unwrap_or(4);
  BindingRuntimeConfig {
    flavor: BindingRuntimeFlavor::MultiThread,
    worker_threads: saturating_u32(worker_threads as u64),
    max_blocking_tasks: saturating_u32(max_blocking_tasks as u64),
  }
}

#[cfg(feature = "async-runtime")]
#[napi]
pub fn get_async_runtime_metrics() -> BindingRuntimeMetrics {
  metrics().into()
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
pub fn get_async_runtime_metrics() -> BindingRuntimeMetrics {
  let config = get_async_runtime_config();
  BindingRuntimeMetrics {
    flavor: config.flavor,
    worker_threads: config.worker_threads,
    max_blocking_tasks: config.max_blocking_tasks,
    tasks_spawned: 0,
    tasks_completed: 0,
    tasks_panicked: 0,
    runnable_schedules: 0,
    runnable_polls: 0,
    queued_runnables: 0,
    max_queued_runnables: 0,
    active_runnables: 0,
    max_active_runnables: 0,
    blocking_tasks_started: 0,
    blocking_tasks_completed: 0,
    active_blocking_tasks: 0,
    max_active_blocking_tasks: 0,
  }
}

#[cfg(feature = "async-runtime")]
#[napi]
pub fn reset_async_runtime_metrics() {
  reset_metrics();
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
pub fn reset_async_runtime_metrics() {}

#[cfg(feature = "async-runtime")]
#[napi_derive::module_init]
fn register_async_runtime() {
  use crate::env_config::resolve_thread_count;

  let mut options = RuntimeOptions::default();
  #[cfg(not(target_family = "wasm"))]
  {
    options.worker_threads = resolve_thread_count(
      std::env::var("ROLLDOWN_WORKER_THREADS").ok(),
      num_cpus::get_physical(),
    );
  }
  options.max_blocking_tasks = resolve_thread_count(
    std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS").ok(),
    options.worker_threads,
  );
  if let Ok(flavor) = std::env::var("ROLLDOWN_RUNTIME") {
    options.flavor = match flavor.as_str() {
      "current" | "current-thread" | "single" | "single-thread" => RuntimeFlavor::CurrentThread,
      "multi" | "multi-thread" => RuntimeFlavor::MultiThread,
      _ => options.flavor,
    };
  }
  configure(options).expect("Failed to configure the Rolldown async runtime");
  create_custom_async_runtime(RolldownAsyncRuntime);
}
