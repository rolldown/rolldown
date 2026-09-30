fn main() {
  use napi_build::setup;
  // wasm32-wasip1 and wasm32-wasip1-threads emit IDENTICAL `rustc --print cfg`
  // sets (same `target_env = "p1"`; `target_feature = "atomics"` is set for
  // NEITHER), so the two can only be told apart via the cargo TARGET.
  println!("cargo::rustc-check-cfg=cfg(rolldown_wasi_threads)");
  if std::env::var("TARGET").as_deref() == Ok("wasm32-wasip1-threads") {
    println!("cargo::rustc-cfg=rolldown_wasi_threads");
    // Route libc's allocation entry points that zero, copy or align through
    // src/wasm_heap_sync.rs (`__wrap_<name>`), so C callers such as emnapi's
    // calloc in `napi_create_async_work` refresh the thread's view of the
    // shared memory size too. `malloc` is left alone: @emnapi/core's JS side
    // requires the module's `malloc` export, and `--wrap=malloc` removes that
    // export ("TypeError: malloc is not exported"). A plain malloc only writes
    // dlmalloc's chunk headers, which V8 checks against the real size.
    // Threaded target only: the single-thread build has one thread, so its
    // view of the memory size is never stale.
    // See internal-docs/wasi-shared-memory-grow/implementation.md
    for symbol in ["calloc", "realloc", "aligned_alloc", "posix_memalign"] {
      println!("cargo::rustc-link-arg=--wrap={symbol}");
    }
  }
  setup();
}
