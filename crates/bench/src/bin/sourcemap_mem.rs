//! Real-filesystem bench for the "avoid unnecessary intermediate sourcemaps"
//! optimization (commit `f6653cb7b`).
//!
//! Bundles the `threejs` fixture with sourcemap output enabled and a transform
//! hook that changes code without returning a sourcemap, repeated several
//! times. Reports per-iteration wall time (min/median) and the process peak
//! working set (OS-level peak RSS).
//!
//! Run this binary built from commit `f6653cb7b` and from its parent
//! `f6653cb7b^` and compare the printed time and peak.
//!
//! Uses the high-level `Bundler` (real OS filesystem) rather than the criterion
//! `MemoryFileSystem` harness, which cannot create `D:\`-rooted paths in its vfs
//! layer on Windows. Disk reads are OS-cached and identical across before/after,
//! so they do not bias the comparison.
//!
//! Usage:
//!   cargo run -p bench --release --bin sourcemap_mem [-- <omitted|null|none> <iterations>]

use bench::{MapMode, omit_map_plugin};
use rolldown::plugin::__inner::SharedPluginable;
use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use std::time::{Duration, Instant};

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() {
  let mut args = std::env::args().skip(1);
  let mode = args.next().unwrap_or_else(|| "omitted".to_string());
  let iterations: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);

  let make_plugins = || -> Vec<SharedPluginable> {
    match mode.as_str() {
      "omitted" => vec![omit_map_plugin(MapMode::Omitted)],
      "null" => vec![omit_map_plugin(MapMode::Null)],
      "none" => vec![],
      other => {
        eprintln!("unknown mode {other:?}; expected omitted|null|none");
        std::process::exit(1);
      }
    }
  };

  let root = rolldown_workspace::root_dir();
  let three_dir = root.join("tmp/bench/three");
  let entry = three_dir.join("entry.js").to_str().unwrap().to_string();

  let mut times: Vec<Duration> = Vec::with_capacity(iterations);
  for i in 0..iterations {
    let options = BundlerOptions {
      input: Some(vec![InputItem { name: Some("threejs".to_string()), import: entry.clone() }]),
      cwd: Some(three_dir.clone()),
      sourcemap: Some(SourceMapType::File),
      ..Default::default()
    };
    let mut bundler =
      Bundler::with_plugins(options, make_plugins()).expect("Failed to create bundler");

    let start = Instant::now();
    let _output = bundler.generate().await.expect("Failed to bundle");
    let elapsed = start.elapsed();
    times.push(elapsed);
    eprintln!("iteration {} done in {:.1} ms", i + 1, elapsed.as_secs_f64() * 1000.0);
  }

  times.sort_unstable();
  let min = times.first().copied().unwrap_or_default();
  let median = times[times.len() / 2];
  let peak = peak_working_set_bytes();

  println!("mode={mode} iterations={iterations}");
  println!("time_min_ms={:.1} time_median_ms={:.1}", min.as_secs_f64() * 1000.0, median.as_secs_f64() * 1000.0);
  println!(
    "peak_working_set_bytes={peak} peak_working_set_mib={:.2}",
    peak as f64 / (1024.0 * 1024.0)
  );
}

/// OS-reported peak resident/working-set size of the current process, in bytes.
/// Returns 0 if the platform is unsupported.
fn peak_working_set_bytes() -> u64 {
  #[cfg(windows)]
  {
    // `K32*` PSAPI functions are exported directly from kernel32.dll, so no
    // extra link directive is required.
    #[repr(C)]
    struct ProcessMemoryCounters {
      cb: u32,
      page_fault_count: u32,
      peak_working_set_size: usize,
      working_set_size: usize,
      quota_peak_paged_pool_usage: usize,
      quota_paged_pool_usage: usize,
      quota_peak_non_paged_pool_usage: usize,
      quota_non_paged_pool_usage: usize,
      pagefile_usage: usize,
      peak_pagefile_usage: usize,
    }

    unsafe extern "system" {
      fn GetCurrentProcess() -> isize;
      fn K32GetProcessMemoryInfo(
        process: isize,
        counters: *mut ProcessMemoryCounters,
        cb: u32,
      ) -> i32;
    }

    unsafe {
      let mut counters: ProcessMemoryCounters = std::mem::zeroed();
      counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
      if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0 {
        counters.peak_working_set_size as u64
      } else {
        0
      }
    }
  }

  #[cfg(target_os = "linux")]
  {
    // VmHWM = peak resident set size, in kB.
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
      for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
          if let Some(kb) = rest.split_whitespace().next().and_then(|n| n.parse::<u64>().ok()) {
            return kb * 1024;
          }
        }
      }
    }
    0
  }

  #[cfg(not(any(windows, target_os = "linux")))]
  {
    0
  }
}
