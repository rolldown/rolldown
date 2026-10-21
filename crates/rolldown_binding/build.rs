fn main() {
  use napi_build::setup;
  // wasm32-wasip1 and wasm32-wasip1-threads emit IDENTICAL `rustc --print cfg`
  // sets (same `target_env = "p1"`; `target_feature = "atomics"` is set for
  // NEITHER), so the two can only be told apart via the cargo TARGET.
  println!("cargo::rustc-check-cfg=cfg(rolldown_wasi_threads)");
  if std::env::var("TARGET").as_deref() == Ok("wasm32-wasip1-threads") {
    println!("cargo::rustc-cfg=rolldown_wasi_threads");
  }
  // On wasm32-wasip1-threads, `setup()` also links napi's allocator lock (napi enables
  // napi-build's `wasi-heap-sync`). The global allocator stays `System`, which ends in
  // wasi-libc's `malloc`, so Rust allocations take the same lock.
  // See internal-docs/async-runtime/implementation.md, "Threaded WASI heap sync".
  setup();
}
