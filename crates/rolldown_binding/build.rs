fn main() {
  use napi_build::setup;
  // wasm32-wasip1 and wasm32-wasip1-threads emit IDENTICAL `rustc --print cfg`
  // sets (same `target_env = "p1"`; `target_feature = "atomics"` is set for
  // NEITHER), so the two can only be told apart via the cargo TARGET.
  println!("cargo::rustc-check-cfg=cfg(rolldown_wasi_threads)");
  if std::env::var("TARGET").as_deref() == Ok("wasm32-wasip1-threads") {
    println!("cargo::rustc-cfg=rolldown_wasi_threads");
    // Route every entry point into wasi-libc's dlmalloc, and its `sbrk`, through
    // src/wasm_heap_sync.rs (`__wrap_<name>`), which takes one lock around each
    // dlmalloc call and refreshes this thread's view of the shared memory size
    // under it. `nm` on the sysroot's self-contained libc.a (rustc 1.98.1):
    //
    // | symbol             | defined in (T) | referenced (U) by                          |
    // |--------------------|----------------|--------------------------------------------|
    // | malloc             | dlmalloc.c.obj | libc (stdio, dirent, pthread_create), emnapi |
    // | free               | dlmalloc.c.obj | libc (same), emnapi                        |
    // | calloc             | dlmalloc.c.obj | libc (environ, preopens, regex), emnapi    |
    // | realloc            | dlmalloc.c.obj | libc (getdelim, glob, reallocarray), emnapi |
    // | posix_memalign     | dlmalloc.c.obj | std's `System` only (no C caller)          |
    // | aligned_alloc      | dlmalloc.c.obj | no caller today                            |
    // | malloc_usable_size | dlmalloc.c.obj | no caller today                            |
    // | __libc_malloc      | malloc alias   | libc locale (duplocale, newlocale)         |
    // | __libc_free        | free alias     | libc locale (freelocale)                   |
    // | __libc_calloc      | calloc alias   | libc atexit                                |
    // | sbrk               | sbrk.c.obj     | dlmalloc.c.obj only                        |
    //
    // Inside dlmalloc.c.obj the public names are thin wrappers over static
    // `dlmalloc` / `dlfree` / ...; its only calls out of the object are `sbrk`
    // and `sched_yield`, so a wrapped entry never re-enters another one.
    // `--wrap=malloc` / `--wrap=free` also renames napi-build's `--export=malloc`
    // / `--export=free`: packages/rolldown/build-binding.ts runs
    // scripts/wasi/rename-wasm-allocator-exports.mjs after the link so that the
    // exports @emnapi/core calls are the locked wrappers.
    // Threaded target only: the single-thread build has one thread, so its
    // view of the memory size is never stale.
    // See internal-docs/wasi-shared-memory-grow/implementation.md
    for symbol in [
      "malloc",
      "free",
      "calloc",
      "realloc",
      "posix_memalign",
      "aligned_alloc",
      "malloc_usable_size",
      "__libc_malloc",
      "__libc_free",
      "__libc_calloc",
      "sbrk",
    ] {
      println!("cargo::rustc-link-arg=--wrap={symbol}");
    }
  }
  setup();
}
