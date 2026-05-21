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
// Output strategy (every step is allocation-free):
//   - Each line is prefixed with `[t=SEC.MICROSEC]` from CLOCK_MONOTONIC, so
//     cross-thread chronology can be reconstructed even when execa/vitest
//     prints stderr and stdout in separate blocks.
//   - Basic line (size/align/thread) goes out first via raw libc::write so
//     it always reaches stderr.
//   - Backtrace via a manual FP-chain walk (alloc-free, signal-safe).
//     Each frame is resolved through dyld's image-list APIs
//     (`_dyld_image_count` / `_dyld_get_image_header` / `_dyld_get_image_name`)
//     to produce `IP  image+0xOFFSET  base=0xBASE` lines. Offline
//     symbolication: `atos -arch arm64 -o <image-path> -l 0x<base> 0x<ip>`.
//     `dladdr` is intentionally avoided — observed to SIGABRT from a
//     wedged-allocator state on macOS.
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

  // dyld image-list APIs (mach-o.dylib). Read-only access to dyld's
  // gAllImages vector — no locks, no allocations. Unlike `dladdr`, these
  // are safe to call from a wedged-allocator state because they don't
  // walk symbol tables or take internal mutexes that dladdr asserts on.
  #[cfg(target_os = "macos")]
  unsafe extern "C" {
    fn _dyld_image_count() -> u32;
    fn _dyld_get_image_header(image_index: u32) -> *const c_void;
    fn _dyld_get_image_name(image_index: u32) -> *const core::ffi::c_char;
  }

  // Find which loaded image contains `addr` via linear scan of the dyld
  // image list. Returns (base, name) for the image with the greatest base
  // <= addr. Bounded by image count (~200 on a typical Node process),
  // microseconds total. Allocation-free.
  #[cfg(target_os = "macos")]
  fn find_image_for(addr: usize) -> Option<(usize, &'static str)> {
    let count = unsafe { _dyld_image_count() };
    let mut best: Option<(usize, &'static str)> = None;
    for i in 0..count {
      let header = unsafe { _dyld_get_image_header(i) };
      if header.is_null() {
        continue;
      }
      let base = header.addr();
      if base > addr {
        continue;
      }
      let name_ptr = unsafe { _dyld_get_image_name(i) };
      if name_ptr.is_null() {
        continue;
      }
      let Ok(name) = (unsafe { CStr::from_ptr(name_ptr) }).to_str() else {
        continue;
      };
      match best {
        None => best = Some((base, name)),
        Some((prev_base, _)) if prev_base < base => best = Some((base, name)),
        _ => {}
      }
    }
    best
  }

  // Print a single frame line: raw IP, plus image basename + offset + base
  // address if we can locate the image. Format optimized for paste into
  // `atos -o <image> -l <base> <ip>` for offline symbolication.
  #[cfg(unix)]
  fn write_frame_line(ip: *mut c_void) {
    let addr = ip.addr();
    let mut buf = [0u8; 512];
    let mut bw = StackBuf { buf: &mut buf, pos: 0 };
    write_timestamp(&mut bw);
    let _ = write!(bw, "  0x{addr:x}");

    #[cfg(target_os = "macos")]
    if let Some((base, name)) = find_image_for(addr) {
      let basename = name.rsplit('/').next().unwrap_or(name);
      let offset = addr.saturating_sub(base);
      let _ = write!(bw, "  {basename}+0x{offset:x}  base=0x{base:x}");
    }

    let _ = writeln!(bw);
    let len = bw.pos;
    write_stderr(&buf[..len]);
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
