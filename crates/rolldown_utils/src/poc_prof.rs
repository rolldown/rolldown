//! PoC-only per-phase CPU accounting for `poc/chunk-ast-minify` (see `POC_REPORT.md`).
//!
//! Enabled with `ROLLDOWN_POC_PROF=1`. Each phase accumulates the calling thread's CPU time
//! (`CLOCK_THREAD_CPUTIME_ID`) into a global counter, so phases that run in parallel report
//! their summed CPU, not wall time.

use std::sync::{
  LazyLock,
  atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy)]
pub enum Phase {
  /// `minify_chunks`: parse of the joined chunk text.
  MinifyParse,
  /// `minify_chunks`: `Minifier::dce`/`minify` (semantic + stats + compressor).
  MinifyDce,
  /// `minify_chunks`: codegen of the minified chunk (+ sourcemap).
  MinifyCodegen,
  /// `minify_chunks`: `collapse_sourcemaps`.
  MinifyCollapse,
  /// Fast path: take module bodies out, parse generated imports/exports, assemble the program.
  AstAssemble,
  /// Fast path: `Minifier::dce` on the assembled chunk program.
  AstDce,
  /// Fast path: split the body back into modules and print the generated imports/exports.
  AstSplit,
  /// Per-module codegen in `create_chunk_to_codegen_ret_map` (summed across threads).
  ModulePrint,
  /// `SourceJoiner::join` (summed across chunks).
  Join,
}

const NAMES: [&str; 9] = [
  "minify.parse",
  "minify.dce",
  "minify.codegen",
  "minify.collapse",
  "ast.assemble",
  "ast.dce",
  "ast.split",
  "module_print",
  "join",
];

static COUNTERS: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];

pub static ENABLED: LazyLock<bool> =
  LazyLock::new(|| std::env::var_os("ROLLDOWN_POC_PROF").is_some_and(|v| v == "1"));

#[cfg(all(unix, not(target_family = "wasm")))]
pub fn thread_cpu_ns() -> u64 {
  #[repr(C)]
  struct Ts(i64, i64);
  unsafe extern "C" {
    fn clock_gettime(clk: i32, ts: *mut Ts) -> i32;
  }
  let mut t = Ts(0, 0);
  // 3 = CLOCK_THREAD_CPUTIME_ID on Linux.
  unsafe { clock_gettime(3, &raw mut t) };
  #[expect(clippy::cast_sign_loss)]
  let ns = (t.0 as u64) * 1_000_000_000 + t.1 as u64;
  ns
}

#[cfg(not(all(unix, not(target_family = "wasm"))))]
pub fn thread_cpu_ns() -> u64 {
  0
}

pub struct Timer(u64);

impl Timer {
  #[inline]
  pub fn start() -> Self {
    Self(if *ENABLED { thread_cpu_ns() } else { 0 })
  }

  #[inline]
  pub fn stop(self, phase: Phase) {
    if *ENABLED {
      COUNTERS[phase as usize].fetch_add(thread_cpu_ns() - self.0, Ordering::Relaxed);
    }
  }
}

/// Returns `name=ms` pairs for non-zero phases and resets the counters.
pub fn report_and_reset() -> String {
  let mut out = String::new();
  for (name, counter) in NAMES.iter().zip(&COUNTERS) {
    let ns = counter.swap(0, Ordering::Relaxed);
    if ns != 0 {
      #[expect(clippy::cast_precision_loss)]
      let ms = ns as f64 / 1e6;
      out.push_str(&format!("{name}={ms:.1}ms "));
    }
  }
  out
}
