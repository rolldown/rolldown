# Async runtime — Design & Principles

## Summary

Rolldown runs its bundling work on one tokio multi-thread runtime.
`crates/rolldown_binding/src/lib.rs` builds that runtime in `module_init`. It
then gives the runtime to NAPI-RS through `create_custom_tokio_runtime`. Plugin
hooks are `async`, and they must return `Send` futures.

The type system does not enforce one more rule. This doc records it: **a hook
must not block its worker thread while it waits for the runtime.**

## The rule

Do not call `rolldown_utils::futures::block_on` in a plugin hook when the awaited
work needs the runtime. Do not use another blocking wait either. Use `.await`.

"Needs the runtime" covers more cases than the words suggest. Two examples:

1. A task from `tokio::spawn`. A worker thread must poll that task.
2. A JS callback that re-enters rolldown. A Vite resolver that routes through a
   plugin container is one such callback. The NAPI-RS promise behind it resolves
   on the same runtime.

One case is safe. Work that only waits for the JS thread to answer a
threadsafe-function call does not need the runtime. The JS thread answers
through a oneshot channel, and the blocked thread polls that channel itself.

## Why it deadlocks

Two pool sizes control the failure:

1. `worker_threads` defaults to `num_cpus::get_physical() * 3 / 2`. The
   `ROLLDOWN_WORKER_THREADS` variable overrides it.
2. `max_blocking_threads` defaults to **4**. The
   `ROLLDOWN_MAX_BLOCKING_THREADS` variable overrides it.

`block_on` calls `tokio::task::block_in_place`. That function moves the worker's
scheduler work to a blocking-pool thread, so the runtime continues. Tokio puts
the hand-off in a queue when the blocking pool is full. Tokio does not make the
pool larger.

Each blocked hook holds one worker thread and one blocking thread. Approximately
`worker_threads + max_blocking_threads` hooks can block together. Above that
number, no thread remains to run the scheduler work. All work that the blocked
hooks wait for then stops forever.

The number is small on CI:

| Machine        | Physical cores | Concurrent blocked hooks before the deadlock |
| -------------- | -------------- | -------------------------------------------- |
| 2-vCPU runner  | 1              | 5                                            |
| 4-vCPU runner  | 2              | 7                                            |
| 14-core laptop | 14             | 25                                           |

This difference explains a common report: the build completes on a laptop, and
it hangs on CI.

Rolldown had this problem in #10664. The `builtin:vite-dynamic-import-vars`
plugin blocked in `transform`. The plugin spawned tasks to call the JS resolver.
Those tasks never ran. For the fix, see
`crates/rolldown_plugin_vite_dynamic_import_vars/src/lib.rs`. It parses the code
in an inner scope. It carries owned data out of that scope. It then awaits.

## The wasm target

`crates/rolldown_binding/src/lib.rs` builds the custom runtime only on targets
that are not wasm. The wasm build uses the NAPI-RS runtime instead, so the pool
sizes above do not apply to it.

The rule still applies. On wasm, `block_on` calls
`futures::executor::block_on`, which holds the calling thread. A spawned task
still needs a worker thread to poll it.

## How to await inside a hook that parses the AST

The oxc AST is `!Send`. A hook therefore cannot hold the AST across an `.await`.
Do not block the thread to avoid this limit. Do these steps instead:

1. Parse the code in an inner scope. Visit the AST in the same scope.
2. Return owned data from that scope. Return spans, offsets, and `String`
   values. Do not return AST references or raw pointers into the arena.
3. Await outside the scope. Then build the result from that data.

`build_edit` in
`crates/rolldown_plugin_vite_dynamic_import_vars/src/ast_visit.rs` shows this
pattern. It takes spans and strings. Both the visit path and the post-await path
therefore call it.

## Unresolved Questions

- `max_blocking_threads` is 4 because rolldown has few true blocking tasks. File
  reads in `crates/rolldown/src/utils/load_source.rs` also use that pool. The
  safety margin is therefore smaller than the number 4 suggests.
- Two issues propose a move from tokio to the NAPI-RS scheduler
  (rolldown#10268 and rolldown#10350). That move would change the pool
  arithmetic on this page. It would not change the rule above.
