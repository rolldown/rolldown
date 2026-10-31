//! PoC-only per-phase CPU accounting for `poc/chunk-ast-minify` (see `POC_REPORT.md`).
//!
//! Enabled with `ROLLDOWN_POC_PROF=1`. Each phase accumulates the calling thread's CPU time
//! (`CLOCK_THREAD_CPUTIME_ID`) into a global counter, so phases that run in parallel report
//! their summed CPU, not wall time.

use std::fmt::Write as _;
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
  /// Profiling-only split of OXC minify: semantic before compression.
  MinifySemantic,
  /// Profiling-only split of OXC minify: normalization and peephole passes.
  MinifyCompress,
  /// Profiling-only split of OXC minify: semantic rebuilt for mangling.
  MinifyMangleSemantic,
  /// Profiling-only split of OXC minify: identifier and private-member mangling.
  MinifyMangle,
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
  /// `global` mode: rebasing module spans into the chunk source space (summed across threads).
  AstRebase,
  /// `global` mode: building the chunk source text and its line tables.
  AstSpace,
  /// `global` mode: remapping module maps from the chunk source space (summed across threads).
  AstRemap,
}

const NAMES: [&str; 16] = [
  "minify.parse",
  "minify.dce",
  "minify.codegen",
  "minify.collapse",
  "minify.semantic",
  "minify.compress",
  "minify.mangle_semantic",
  "minify.mangle",
  "ast.assemble",
  "ast.dce",
  "ast.split",
  "module_print",
  "join",
  "ast.rebase",
  "ast.space",
  "ast.remap",
];

static COUNTERS: [AtomicU64; 16] = [const { AtomicU64::new(0) }; 16];

pub static ENABLED: LazyLock<bool> =
  LazyLock::new(|| std::env::var_os("ROLLDOWN_POC_PROF").is_some_and(|v| v == "1"));

#[cfg(all(unix, not(target_family = "wasm")))]
pub fn thread_cpu_ns() -> u64 {
  let mut time = libc::timespec { tv_sec: 0, tv_nsec: 0 };
  // SAFETY: `time` is a valid writable `timespec` for the duration of the call.
  if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &raw mut time) } != 0 {
    return 0;
  }
  #[expect(clippy::cast_sign_loss)]
  let ns = (time.tv_sec as u64) * 1_000_000_000 + time.tv_nsec as u64;
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
      let _ = write!(out, "{name}={ms:.1}ms ");
    }
  }
  out
}
