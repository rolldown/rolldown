# Memory reclaim

## Summary

Rolldown keeps long-lived worker threads and uses mimalloc. Freed memory stays resident until the thread that owns its page runs mimalloc again, so an idle process can hold gigabytes with almost nothing live. This feature returns that memory once Rolldown is idle. Builds that run back to back keep it until the last one ends. See [design.md](./design.md) for why it works this way.

## Components

- `crates/rolldown/src/utils/build_activity.rs`: a process-wide count of running builds and a hook. `Bundle` and the HMR entries in `impl_bundler_hmr.rs` hold a `BuildActivity`, so `rolldown()`, watch and dev are all covered by one counter. Core knows nothing about mimalloc.
- `crates/rolldown_binding/src/memory_reclaim.rs`: owns the `rolldown-reclaim` thread, created on the first notification, never at import. Each JS thread that creates a bundler, watcher or dev engine registers a weak threadsafe function, so a round reaches that JS thread too.
- `BindingOutputChunk` and `BindingOutputAsset`: report their size to V8 through `napi_adjust_external_memory` and notify when finalized.
- `ClassicBundler::close` and `BundleHandle::close` notify once they drop the build's plugin driver.

## Data flow

1. The last `BuildActivity` drops, or a finalizer or `close()` releases build data. Each one calls `notify_memory_released`.
2. `rolldown-reclaim` waits for a quiet window (`ROLLDOWN_MEMORY_RECLAIM_DELAY`, 500 ms by default, `off` disables) with no active build. While a build runs, it sleeps until the next notification, since the last build to end always sends one.
3. Every tokio worker, blocking thread, rayon worker and registered JS thread calls `mi_theap_quiescent_abandon` on its own theap. It settles the frees other threads queued on its pages, returns its empty pages, and gives up the pages that still hold live data. Then `mi_arenas_purge` returns the free memory to the OS. Module sources are read on blocking threads, so skipping them leaves much of a build resident.
4. When JS frees the output later, the free that empties a given-up page returns it to the arena on the freeing thread, so the thread that made the data is not needed again. The finalizers notify, and the next round's purge returns that memory to the OS.
5. A round that misses a tokio or blocking thread runs again once the process has been quiet for the window plus 1, 2, 4 and 8 s.

## Invariants

- Each round must reach every thread. A thread waits until every thread has taken its task, so the remaining tasks go to other threads, and settles after that wait. Tasks that only yield reach about half of the tokio workers. The wait has a 100 ms deadline, because an unbounded `Barrier` deadlocks with a worker stuck in a transform-time `block_on`.
- mimalloc must be 3.6 or newer, which added `mi_theap_quiescent_abandon`. Otherwise reclaim stays off.
- `libmimalloc-sys2` declares none of the 3.6 functions and options used here, and its option numbers are from 2.x. So `memory_reclaim.rs` declares them itself, matching `mimalloc.h` of the version in `CHECKED_MIMALLOC`. A debug build panics at load with any other version, and CI tests a debug build, so a mimalloc update has to check them again. A release build with another version skips the options.
- `mi_theap_quiescent_abandon` runs only on the thread that owns the theap, outside any other mimalloc call: in a task, a broadcast job or a JS callback. Its header also allows it for a foreign thread that is quiescent, which Rolldown cannot prove.
- The JS thread's call usually lands after the round's purge, so that thread purges again.
- mimalloc options are written once at load, before the async runtime starts its threads. They are written as defaults, so a `MIMALLOC_*` environment variable still wins. Only options mimalloc reads at use time can be set this way: `page_reclaim_on_free` is copied into the main heap at process start, so setting it at load does nothing.
- `page_commit_on_demand = 2` only changes systems without overcommit: Windows, and Linux with `vm.overcommit_memory=2`. Elsewhere mimalloc still commits pages eagerly.
- Never start threads from `module_init`. On Linux it runs under the loader lock, and a new thread that touches dynamic TLS waits for that lock (this deadlocked with `rayon::ThreadPoolBuilder::build_global`).
- Memory JS still holds cannot be reclaimed yet. Its finalizer triggers another round.

## Gaps

libuv threads (`enhancedTransform`) are not settled. A wrapper type that does not notify delays the purge until the next round, it does not leak. A thread that stays busy through every retry keeps its pages until the next round.

## Measuring

On macOS, read the footprint with `proc_pid_rusage` (`ri_phys_footprint`). `vmmap` reads the process, and when it runs during a round, the purged pages stay counted as DIRTY afterwards. That was the "about 1 in 15 runs keeps 1.3 GB" outlier seen before. RSS on macOS does not drop after a purge at all.

Walk the output inside a function, and check with a `FinalizationRegistry` that every output was collected. A loop at the top level of a module keeps its last items alive, since the suspended async frame holds stale registers. That kept a whole build's output alive and once looked like memory mimalloc would not return.
