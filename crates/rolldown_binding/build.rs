fn main() {
  use napi_build::setup;
  // wasm32-wasip1 and wasm32-wasip1-threads emit IDENTICAL `rustc --print cfg`
  // sets (same `target_env = "p1"`; `target_feature = "atomics"` is set for
  // NEITHER), so the two can only be told apart via the cargo TARGET.
  println!("cargo::rustc-check-cfg=cfg(rolldown_wasi_threads)");
  if std::env::var("TARGET").as_deref() == Ok("wasm32-wasip1-threads") {
    println!("cargo::rustc-cfg=rolldown_wasi_threads");
  }
  // On wasm32-wasip1-threads, `setup()` also links napi's allocator lock (the `--wrap` link
  // args for wasi-libc's malloc family and `sbrk`, plus the `malloc` / `free` export shim):
  // napi turns on napi-build's `wasi-heap-sync` feature. The Rust global allocator stays
  // `System`, which ends in wasi-libc's `malloc`, so it goes through the same lock.
  // See internal-docs/wasi-shared-memory-grow/implementation.md
  setup();
}
