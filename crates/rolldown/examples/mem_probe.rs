//! Memory probe for incremental builds (review of memory-optimization claims).
//!
//! Runs a full build with `experimental.incremental_build = true`, then partial
//! rebuilds, printing live/peak heap bytes at each checkpoint, plus a
//! non-incremental baseline.
//!
//! Build & run (needs `just setup-bench` fixtures in `tmp/bench`):
//!   cargo build --release -p rolldown --features experimental --example mem_probe
//!   ./target/release/examples/mem_probe rome
//!   ./target/release/examples/mem_probe three10x
//!
//! Placed in `crates/rolldown/examples` (not `crates/bench`) because
//! `rolldown_common::ScanMode` is not re-exported by `rolldown`, and the bench
//! crate cannot name it. A counting allocator is embedded instead of
//! `rolldown_tracking_allocator` for the same reason.

// A measurement tool, not library code: it prints its report and rounds bytes to MiB.
#![allow(
  clippy::print_stdout,
  clippy::cast_precision_loss,
  clippy::unreadable_literal,
  clippy::identity_op,
  clippy::redundant_closure_for_method_calls
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use rolldown::{Bundler, BundlerOptions, ExperimentalOptions, InputItem, TsConfig};
use rolldown_common::ScanMode;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

fn add(n: usize) {
  let v = LIVE.fetch_add(n, Relaxed) + n;
  PEAK.fetch_max(v, Relaxed);
}
fn sub(n: usize) {
  LIVE.fetch_sub(n, Relaxed);
}

unsafe impl GlobalAlloc for Counting {
  unsafe fn alloc(&self, l: Layout) -> *mut u8 {
    let p = unsafe { System.alloc(l) };
    if !p.is_null() {
      add(l.size());
    }
    p
  }
  unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
    let p = unsafe { System.alloc_zeroed(l) };
    if !p.is_null() {
      add(l.size());
    }
    p
  }
  unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
    unsafe { System.dealloc(p, l) };
    sub(l.size());
  }
  unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
    let np = unsafe { System.realloc(p, l, new) };
    if !np.is_null() {
      if new > l.size() {
        add(new - l.size());
      } else {
        sub(l.size() - new);
      }
    }
    np
  }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn live() -> usize {
  LIVE.load(Relaxed)
}
fn reset_peak() {
  PEAK.store(LIVE.load(Relaxed), Relaxed);
}
fn peak() -> usize {
  PEAK.load(Relaxed)
}
fn mib(b: usize) -> f64 {
  b as f64 / 1048576.0
}

fn options(fixture: &str, incremental: bool) -> (BundlerOptions, &'static str) {
  let root = rolldown_workspace::root_dir();
  let experimental =
    Some(ExperimentalOptions { incremental_build: Some(incremental), ..Default::default() });
  match fixture {
    "rome" => (
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("rome-ts".to_string()),
          import: root.join("tmp/bench/rome/src/entry.ts").to_str().unwrap().to_string(),
        }]),
        cwd: Some(root.join("tmp/bench/rome")),
        shim_missing_exports: Some(true),
        tsconfig: Some(TsConfig::Manual(root.join("tmp/bench/rome/src/tsconfig.json"))),
        experimental,
        ..Default::default()
      },
      "/@romejs/js-parser/",
    ),
    "three10x" => (
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("threejs10x".to_string()),
          import: root.join("tmp/bench/three10x/entry.js").to_str().unwrap().to_string(),
        }]),
        cwd: Some(root.join("tmp/bench/three10x")),
        experimental,
        ..Default::default()
      },
      "/copy1/math/",
    ),
    "three" => (
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("threejs".to_string()),
          import: root.join("tmp/bench/three/entry.js").to_str().unwrap().to_string(),
        }]),
        cwd: Some(root.join("tmp/bench/three")),
        experimental,
        ..Default::default()
      },
      "/src/math/",
    ),
    other => panic!("unknown fixture {other}"),
  }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() {
  let fixture = std::env::args().nth(1).unwrap_or_else(|| "rome".to_string());
  let (opts, changed_hint) = options(&fixture, true);

  println!("=== fixture: {fixture} ===");
  println!("[0] process start                      live={:8.1} MiB", mib(live()));

  let mut bundler = Bundler::new(opts).expect("bundler");
  println!("[1] after Bundler::new                 live={:8.1} MiB", mib(live()));

  // Build 1: full scan with incremental cache enabled (make_copy clones all ASTs).
  reset_peak();
  let t = Instant::now();
  let out = bundler.generate().await.expect("build 1");
  let d1 = t.elapsed();
  let live1 = live();
  println!(
    "[2] after full incremental build       live={:8.1} MiB  peak={:8.1} MiB  (transient {:7.1} MiB)  {:?}  assets={} warnings={}",
    mib(live1),
    mib(peak()),
    mib(peak() - 0),
    d1,
    out.assets.len(),
    out.warnings.len()
  );
  drop(out);
  let live1b = live();
  println!(
    "[2b] after dropping BundleOutput        live={:8.1} MiB  (steady state: cache + module_infos + resolver)",
    mib(live1b)
  );

  // Size of what the plugin-facing module_infos retain (relevant to C5).
  let infos = bundler.module_infos();
  let n = infos.len();
  let code_bytes: usize =
    infos.iter().map(|e| e.value().code.as_ref().map_or(0, |c| c.len())).sum();
  println!("     module_infos: {n} modules, code bytes = {:.1} MiB", mib(code_bytes));

  // Pick a changed file for the partial rebuilds.
  let mut candidates: Vec<String> = infos
    .iter()
    .map(|e| e.key().to_string())
    .filter(|k| k.contains(changed_hint) && (k.ends_with(".ts") || k.ends_with(".js")))
    .collect();
  candidates.sort();
  let changed = candidates.first().cloned().expect("a module matching the hint");
  drop(infos);
  println!("     changed file for partial rebuilds: {changed}");

  for i in 0..3 {
    let before = live();
    reset_peak();
    let t = Instant::now();
    let out = bundler
      .incremental_generate(ScanMode::Partial(vec![changed.as_str().into()]))
      .await
      .expect("partial rebuild");
    let d = t.elapsed();
    let p = peak();
    let after = live();
    println!(
      "[3.{i}] partial rebuild                   before={:8.1} MiB  peak={:8.1} MiB  (transient {:7.1} MiB)  after(with output)={:8.1} MiB  {:?}  assets={}",
      mib(before),
      mib(p),
      mib(p - before),
      mib(after),
      d,
      out.assets.len()
    );
    drop(out);
    println!("       after dropping output            live={:8.1} MiB", mib(live()));
  }

  // Full rescan with the cache present (IncrementalFullBuild: make_copy path).
  {
    let before = live();
    reset_peak();
    let t = Instant::now();
    let out = bundler.generate().await.expect("full rebuild");
    let d = t.elapsed();
    println!(
      "[4] full rebuild (cache present)       before={:8.1} MiB  peak={:8.1} MiB  (transient {:7.1} MiB)  {:?}  assets={}",
      mib(before),
      mib(peak()),
      mib(peak() - before),
      d,
      out.assets.len()
    );
    drop(out);
    println!("       after dropping output            live={:8.1} MiB", mib(live()));
  }

  bundler.close().await.expect("close");
  drop(bundler);
  println!("[5] after close + drop(bundler)        live={:8.1} MiB", mib(live()));

  // Baseline: same build with incremental cache disabled.
  let (opts, _) = options(&fixture, false);
  let mut bundler = Bundler::new(opts).expect("bundler");
  let before = live();
  reset_peak();
  let t = Instant::now();
  let out = bundler.generate().await.expect("non-incremental build");
  let d = t.elapsed();
  println!(
    "[6] non-incremental full build         before={:8.1} MiB  peak={:8.1} MiB  (transient {:7.1} MiB)  after(with output)={:8.1} MiB  {:?}  assets={}",
    mib(before),
    mib(peak()),
    mib(peak() - before),
    mib(live()),
    d,
    out.assets.len()
  );
  drop(out);
  println!("       after dropping output            live={:8.1} MiB", mib(live()));
  bundler.close().await.expect("close");
  drop(bundler);
  println!("[7] after close + drop(bundler)        live={:8.1} MiB", mib(live()));
}
