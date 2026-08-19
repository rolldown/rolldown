use napi_derive::napi;

/// Counters of the Rust-side tracking allocator. V8 never allocates through
/// the Rust global allocator, so these numbers exclude the JS heap and GC
/// noise completely, unlike `process.memoryUsage()`.
#[napi(object, object_from_js = false)]
pub struct BindingNativeMemoryStats {
  /// Bytes currently allocated and not yet freed, since process start.
  pub live_bytes: f64,
  /// Highest `live_bytes` seen since process start or the last reset.
  pub peak_bytes: f64,
  /// Successful `alloc` calls since the last reset.
  pub alloc_count: f64,
  /// Successful `realloc` calls since the last reset.
  pub realloc_count: f64,
}

/// Whether `TrackingAllocator` is installed as the global allocator. Must
/// match the `#[global_allocator]` cfg in `lib.rs` exactly — otherwise the
/// functions below would report zeros instead of `None`.
const TRACKING_ALLOCATOR_INSTALLED: bool = cfg!(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos"),
  feature = "tracking_allocator"
));

/// Returns the Rust-side allocator counters, or `None` when this binding was
/// built without the `tracking_allocator` cargo feature (the default —
/// tracking costs a few atomic operations per allocation).
#[napi]
pub fn get_native_memory_stats() -> Option<BindingNativeMemoryStats> {
  TRACKING_ALLOCATOR_INSTALLED.then(collect_stats)
}

/// Starts a new measuring window: the peak restarts from the current live
/// bytes and the counts restart from zero. No-op when the binding was built
/// without the `tracking_allocator` cargo feature.
#[napi]
pub fn reset_native_memory_stats() {
  #[cfg(all(
    not(target_family = "wasm"),
    not(feature = "default_global_allocator"),
    not(target_env = "ohos"),
    feature = "tracking_allocator"
  ))]
  rolldown_tracking_allocator::reset();
}

fn collect_stats() -> BindingNativeMemoryStats {
  #[cfg(all(
    not(target_family = "wasm"),
    not(feature = "default_global_allocator"),
    not(target_env = "ohos"),
    feature = "tracking_allocator"
  ))]
  {
    let stats = rolldown_tracking_allocator::stats();
    #[expect(
      clippy::cast_precision_loss,
      reason = "byte and call counts stay far below 2^53, where f64 is exact"
    )]
    BindingNativeMemoryStats {
      live_bytes: stats.live_bytes as f64,
      peak_bytes: stats.peak_bytes as f64,
      alloc_count: stats.alloc_count as f64,
      realloc_count: stats.realloc_count as f64,
    }
  }
  // Unreachable: the only caller gates on `TRACKING_ALLOCATOR_INSTALLED`.
  #[cfg(not(all(
    not(target_family = "wasm"),
    not(feature = "default_global_allocator"),
    not(target_env = "ohos"),
    feature = "tracking_allocator"
  )))]
  BindingNativeMemoryStats {
    live_bytes: 0.0,
    peak_bytes: 0.0,
    alloc_count: 0.0,
    realloc_count: 0.0,
  }
}
