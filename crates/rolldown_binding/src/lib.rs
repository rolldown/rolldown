#![expect(clippy::print_stderr)]
// Allow type complexity rule, because NAPI-RS requires the direct types to generate the TypeScript definitions.
#![allow(clippy::type_complexity)]
// Due to the bound of NAPI-RS, we need to use `String` though we only need `&str`.
#![allow(clippy::needless_pass_by_value)]
// Most of transmute are just change the lifetime `'a` to `'static`., the annotation, e.g.
//
// BindingTransformPluginContext::new(unsafe {
//   std::mem::transmute::<
//     &rolldown_plugin::TransformPluginContext<'_>,
//     &rolldown_plugin::TransformPluginContext<'_>,
//   >(ctx)
// }),
// Looks redundant
#![allow(clippy::missing_transmute_annotations)]

#[cfg(all(target_family = "wasm", tokio_unstable))]
use std::sync::{
  LazyLock,
  atomic::{AtomicU32, Ordering},
};

use napi_derive::napi;

// Diagnostic global allocator: wraps mimalloc-safe so we can intercept NULL
// returns and print thread, layout, monotonic timestamp, and a backtrace
// with image-relative offsets before aborting. Avoids Rust's default
// "memory allocation of N bytes failed → panic → format panic message
// (allocates) → fails again → skipping backtrace to avoid recursion →
// SIGABRT" spiral, which hides the information needed to root-cause
// allocator failures (e.g. the macOS FIXED_SLOT/DYNAMIC_PTHREADS issue).
//
// Output strategy (the abort path is allocation-free and dyld-free):
//   - Each line is prefixed with `[t=SEC.MICROSEC]` from CLOCK_MONOTONIC, so
//     cross-thread chronology can be reconstructed even when execa/vitest
//     prints stderr and stdout in separate blocks.
//   - Basic line (size/align/thread) goes out first via raw libc::write so
//     it always reaches stderr.
//   - Backtrace via a manual FP-chain walk (alloc-free, signal-safe).
//     Each frame prints two independent writes:
//       1. `  0x<ip>` — guaranteed flush, no library calls
//       2. `      -> <image>+0x<offset>  base=0x<base>` — best-effort,
//          uses a *cached* image table populated once at module load
//          (see `init_image_cache`).
//   - `dladdr` and live `_dyld_image_count` calls are INTENTIONALLY avoided
//     — both have been observed to SIGABRT the process when called from
//     a wedged-allocator state on macOS. The cache snapshots the image
//     list while dyld is still healthy.
//   - Offline symbolication of a raw IP:
//     `atos -arch arm64 -o <image-path> -l 0x<base> 0x<ip>`
#[cfg(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos")
))]
mod diag_alloc {
  use core::ffi::{CStr, c_void};
  use core::fmt::Write as _;
  use std::alloc::{GlobalAlloc, Layout};
  use std::sync::atomic::{AtomicBool, Ordering};

  pub struct DiagAlloc(pub mimalloc_safe::MiMalloc);

  static IN_DIAG: AtomicBool = AtomicBool::new(false);

  // Manual frame-pointer chain walker. Apple Silicon AAPCS64 *requires* FP
  // preservation; SysV AMD64 typically has it under default Rust release
  // profile. Pure asm + load — no libraries, no allocations, signal-safe.
  // Preferred over libSystem's `backtrace()`, which has been observed to
  // return zero frames on certain napi/libuv-spawned threads on macOS
  // (presumably because libunwind cannot locate the stack base for threads
  // started outside the standard pthread path).
  // Frame layout for both arches:
  //   [fp+0]              = saved previous frame pointer
  //   [fp+size_of::<usize>] = saved return address
  #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
  fn capture_stack(buf: &mut [*mut c_void]) -> usize {
    let mut fp: *const usize;
    #[cfg(target_arch = "aarch64")]
    unsafe {
      core::arch::asm!("mov {}, x29", out(reg) fp, options(nomem, nostack, preserves_flags));
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
      core::arch::asm!("mov {}, rbp", out(reg) fp, options(nomem, nostack, preserves_flags));
    }

    let mut n = 0usize;
    while !fp.is_null() && n < buf.len() {
      if (fp.addr()) & (core::mem::align_of::<usize>() - 1) != 0 {
        break; // misaligned, stop rather than fault
      }
      let prev_fp = unsafe { fp.read_volatile() };
      let return_addr = unsafe { fp.add(1).read_volatile() };
      if return_addr == 0 {
        break;
      }
      buf[n] = core::ptr::without_provenance_mut::<c_void>(return_addr);
      n += 1;
      if prev_fp == 0 || prev_fp <= fp.addr() {
        break; // end of stack or invalid back-pointer
      }
      fp = core::ptr::without_provenance::<usize>(prev_fp);
    }
    n
  }

  // Fallback for other arches: libSystem/glibc execinfo backtrace.
  #[cfg(all(
    any(target_os = "macos", target_os = "linux"),
    not(any(target_arch = "aarch64", target_arch = "x86_64"))
  ))]
  fn capture_stack(buf: &mut [*mut c_void]) -> usize {
    use core::ffi::c_int;
    unsafe extern "C" {
      fn backtrace(buffer: *mut *mut c_void, size: c_int) -> c_int;
    }
    let n = unsafe { backtrace(buf.as_mut_ptr(), c_int::try_from(buf.len()).unwrap_or(0)) };
    usize::try_from(n).unwrap_or(0)
  }

  // Allocation-free fmt sink that fills a fixed-size stack buffer and
  // silently truncates on overflow. Used so the diagnostic path never
  // touches the (possibly wedged) global allocator.
  struct StackBuf<'a> {
    buf: &'a mut [u8],
    pos: usize,
  }

  impl core::fmt::Write for StackBuf<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
      let bytes = s.as_bytes();
      let avail = self.buf.len().saturating_sub(self.pos);
      let n = bytes.len().min(avail);
      self.buf[self.pos..self.pos + n].copy_from_slice(&bytes[..n]);
      self.pos += n;
      Ok(())
    }
  }

  #[cfg(unix)]
  fn write_stderr(bytes: &[u8]) {
    unsafe {
      libc::write(2, bytes.as_ptr().cast::<c_void>(), bytes.len());
    }
  }

  #[cfg(not(unix))]
  fn write_stderr(bytes: &[u8]) {
    use std::io::Write as _;
    let _ = std::io::stderr().write_all(bytes);
  }

  // Wall-clock monotonic timestamp prefix so cross-thread ordering (stderr
  // diag vs stdout from other threads) can be reconstructed even when the
  // capturing tool (execa/vitest) prints stderr and stdout in separate
  // blocks. Format: `[t=SEC.MICROSEC] `. Falls back to empty on failure.
  #[cfg(unix)]
  fn write_timestamp(bw: &mut StackBuf<'_>) {
    let mut ts: libc::timespec = unsafe { core::mem::zeroed() };
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &raw mut ts) };
    if rc == 0 {
      let _ = write!(bw, "[t={}.{:06}] ", ts.tv_sec, ts.tv_nsec / 1000);
    }
  }

  #[cfg(not(unix))]
  fn write_timestamp(_bw: &mut StackBuf<'_>) {}

  // dyld image-list APIs (libSystem). Called *only* from
  // `init_image_cache()` at module load time, never from the abort path.
  // Empirically `_dyld_image_count` and friends will SIGABRT the process
  // when called from a wedged-allocator state on macOS (same failure mode
  // as `dladdr`), so we cache results once when dyld is healthy.
  #[cfg(target_os = "macos")]
  unsafe extern "C" {
    fn _dyld_image_count() -> u32;
    fn _dyld_get_image_header(image_index: u32) -> *const c_void;
    fn _dyld_get_image_name(image_index: u32) -> *const core::ffi::c_char;
  }

  #[derive(Copy, Clone)]
  struct ImageEntry {
    base: usize,
    // dyld's image-name strings live for the lifetime of the process;
    // safe to borrow as &'static.
    name: &'static str,
  }

  /// Cached image table populated once at module load. After init, lookups
  /// in the abort path are pure slice reads — no library calls, no locks,
  /// no allocations, no dyld touch.
  #[cfg(target_os = "macos")]
  static IMAGE_TABLE: std::sync::OnceLock<Vec<ImageEntry>> = std::sync::OnceLock::new();

  /// Populate `IMAGE_TABLE` from the current dyld image list. MUST be
  /// invoked from `module_init` (or any other early, healthy context)
  /// before the first DiagAlloc::alloc failure. Safe to call multiple
  /// times — OnceLock ensures only the first call runs.
  #[cfg(target_os = "macos")]
  pub fn init_image_cache() {
    IMAGE_TABLE.get_or_init(|| {
      let count = unsafe { _dyld_image_count() };
      let mut entries: Vec<ImageEntry> = Vec::with_capacity(count as usize);
      for i in 0..count {
        let header = unsafe { _dyld_get_image_header(i) };
        if header.is_null() {
          continue;
        }
        let name_ptr = unsafe { _dyld_get_image_name(i) };
        if name_ptr.is_null() {
          continue;
        }
        let cstr = unsafe { CStr::from_ptr(name_ptr) };
        let Ok(name) = cstr.to_str() else { continue };
        // SAFETY: dyld retains the name string for process lifetime; we
        // only extend the borrow lifetime, not the underlying memory.
        let name_static: &'static str = unsafe { core::mem::transmute::<&str, &'static str>(name) };
        entries.push(ImageEntry { base: header.addr(), name: name_static });
      }
      entries
    });
  }

  /// No-op stub on non-macOS so callers can be unconditional.
  #[cfg(not(target_os = "macos"))]
  pub fn init_image_cache() {}

  /// Lookup `addr`'s containing image in the cached table. Pure memory
  /// reads — no library calls, signal-safe, abort-path-safe. Returns
  /// `None` if the cache was never initialized or no image contains addr.
  #[cfg(target_os = "macos")]
  fn lookup_cached_image(addr: usize) -> Option<(usize, &'static str)> {
    let table = IMAGE_TABLE.get()?;
    let mut best: Option<(usize, &'static str)> = None;
    for e in table {
      if e.base > addr {
        continue;
      }
      match best {
        None => best = Some((e.base, e.name)),
        Some((prev, _)) if prev < e.base => best = Some((e.base, e.name)),
        _ => {}
      }
    }
    best
  }

  /// Print one frame as TWO independent `libc::write` calls:
  ///   1. Always: timestamp + `  0x<ip>\n` — guaranteed to land
  ///   2. Best-effort: `      -> <image>+0x<offset>  base=0x<base>\n`
  ///      iff the cached lookup succeeds. Cache lookup is pure memory
  ///      reads so cannot crash; the worst case is no image cache (lookup
  ///      returns None) and we just skip the second line.
  ///
  /// Splitting the writes guarantees that even if pass 2 crashed (it
  /// can't, but defense-in-depth), the raw IP from pass 1 is already on
  /// the wire — so we never lose the IP regardless.
  #[cfg(unix)]
  fn write_frame_line(ip: *mut c_void) {
    let addr = ip.addr();

    // Pass 1: raw IP — flushed immediately.
    {
      let mut buf = [0u8; 64];
      let mut bw = StackBuf { buf: &mut buf, pos: 0 };
      write_timestamp(&mut bw);
      let _ = writeln!(bw, "  0x{addr:x}");
      let len = bw.pos;
      write_stderr(&buf[..len]);
    }

    // Pass 2: image + offset from cached table (no live dyld calls).
    #[cfg(target_os = "macos")]
    if let Some((base, name)) = lookup_cached_image(addr) {
      let basename = name.rsplit('/').next().unwrap_or(name);
      let offset = addr.saturating_sub(base);
      let mut sbuf = [0u8; 256];
      let mut sw = StackBuf { buf: &mut sbuf, pos: 0 };
      let _ = writeln!(sw, "      -> {basename}+0x{offset:x}  base=0x{base:x}");
      let len = sw.pos;
      write_stderr(&sbuf[..len]);
    }
  }

  #[cold]
  #[inline(never)]
  fn report_and_abort(op: &str, layout: Layout) -> ! {
    if IN_DIAG.swap(true, Ordering::SeqCst) {
      std::process::abort();
    }

    // 1. Basic line — always reaches stderr (raw libc::write, no alloc).
    let t = std::thread::current();
    let mut buf = [0u8; 512];
    let mut bw = StackBuf { buf: &mut buf, pos: 0 };
    write_timestamp(&mut bw);
    let _ = writeln!(
      bw,
      "[alloc-diag] {} FAILED size={} align={} thread_id={:?} name={:?}",
      op,
      layout.size(),
      layout.align(),
      t.id(),
      t.name().unwrap_or("<unnamed>"),
    );
    let len = bw.pos;
    write_stderr(&buf[..len]);

    // 2. Backtrace. Each line: raw IP + image basename + offset + image
    //    base, formatted for offline `atos -o <image> -l <base> <ip>`.
    //    Uses dyld's image-list APIs instead of `dladdr` because the
    //    latter has been observed to SIGABRT on the first call when run
    //    from a wedged-allocator state (libdyld internal assert).
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
      let mut frames: [*mut c_void; 64] = [core::ptr::null_mut(); 64];
      let n = capture_stack(&mut frames);

      let mut hdr = [0u8; 96];
      let mut hbw = StackBuf { buf: &mut hdr, pos: 0 };
      let _ = writeln!(hbw, "[alloc-diag] backtrace ({n} frames):");
      let hlen = hbw.pos;
      write_stderr(&hdr[..hlen]);

      for ip in &frames[..n] {
        write_frame_line(*ip);
      }
    }

    std::process::abort();
  }

  unsafe impl GlobalAlloc for DiagAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
      let p = unsafe { self.0.alloc(layout) };
      if p.is_null() {
        report_and_abort("alloc", layout);
      }
      p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
      let p = unsafe { self.0.alloc_zeroed(layout) };
      if p.is_null() {
        report_and_abort("alloc_zeroed", layout);
      }
      p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
      unsafe { self.0.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
      let p = unsafe { self.0.realloc(ptr, layout, new_size) };
      if p.is_null() {
        let new_layout = Layout::from_size_align(new_size, layout.align()).unwrap_or(layout);
        report_and_abort("realloc", new_layout);
      }
      p
    }
  }
}

#[cfg(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos")
))]
#[global_allocator]
static ALLOC: diag_alloc::DiagAlloc = diag_alloc::DiagAlloc(mimalloc_safe::MiMalloc);

pub mod binding_bundler;
pub mod binding_dev_engine;
pub mod binding_dev_options;
pub mod binding_watcher_bundler;
pub mod classic_bundler;
mod generated;
pub mod options;
pub mod parallel_js_plugin_registry;
pub mod transform;
pub mod transform_cache;
pub mod types;
pub mod utils;
pub mod watcher;
pub mod worker_manager;

// --- External NAPI-RS dependencies ---
pub use oxc_parser_napi;
pub use oxc_resolver_napi;

#[cfg(all(target_family = "wasm", tokio_unstable))]
pub static ACTIVE_TASK_COUNT: LazyLock<AtomicU32> = LazyLock::new(|| AtomicU32::new(1));

#[napi]
/// Shutdown the tokio runtime manually.
///
/// This is required for the wasm target with `tokio_unstable` cfg.
/// In the wasm runtime, the `park` threads will hang there until the tokio::Runtime is shutdown.
pub fn shutdown_async_runtime() {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    if ACTIVE_TASK_COUNT.load(Ordering::Relaxed) > 0 {
      if ACTIVE_TASK_COUNT.fetch_sub(1, Ordering::Relaxed) == 1 {
        napi::bindgen_prelude::shutdown_async_runtime();
      }
    }
  }
}

#[napi]
/// Start the async runtime manually.
///
/// This is required when the async runtime is shutdown manually.
/// Usually it's used in test.
pub fn start_async_runtime() {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    napi::bindgen_prelude::start_async_runtime();
    ACTIVE_TASK_COUNT.fetch_add(1, Ordering::Relaxed);
  }
}

#[napi_derive::module_init]
fn init() {
  // Snapshot dyld's image list while it is still healthy. The abort path
  // (DiagAlloc::report_and_abort) reads from this static cache so it never
  // has to touch dyld in a wedged-allocator state — both `dladdr` and
  // `_dyld_image_count` have been observed to SIGABRT in that state.
  #[cfg(all(
    not(target_family = "wasm"),
    not(feature = "default_global_allocator"),
    not(target_env = "ohos")
  ))]
  diag_alloc::init_image_cache();

  #[cfg(not(target_family = "wasm"))]
  {
    use napi::{bindgen_prelude::create_custom_tokio_runtime, tokio};
    let max_blocking_threads = std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS")
      .ok()
      .and_then(|v| v.parse::<usize>().ok())
      // default value in tokio implementation is **512**
      // it's too high for us
      // we don't have that many `blocking` tasks to run at this moment
      .unwrap_or(4);
    let worker_threads = std::env::var("ROLLDOWN_WORKER_THREADS")
      .ok()
      .and_then(|v| v.parse::<usize>().ok())
      // unlike the web server scenario
      // rolldown puts a lot of blocking tasks in the worker threads rather than blocking_threads
      // so we need to increase the worker threads rather than the blocking_threads
      .unwrap_or(num_cpus::get_physical() * 3 / 2);
    let mut builder = tokio::runtime::Builder::new_multi_thread();

    let rt = builder
      .max_blocking_threads(max_blocking_threads)
      .worker_threads(worker_threads)
      .thread_name("rolldown-worker")
      .enable_all()
      .build()
      .expect("Failed to create tokio runtime");
    create_custom_tokio_runtime(rt);
  }

  #[cfg(not(feature = "disable_panic_hook"))]
  {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
      eprintln!("Rolldown panicked. This is a bug in Rolldown, not your code.");
      default_hook(info);
      eprintln!(
        "\nPlease report this issue at: https://github.com/rolldown/rolldown/issues/new?template=panic_report.yml"
      );
    }));
  }
}
