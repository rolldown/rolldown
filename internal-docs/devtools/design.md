# Devtools — Design & Future Directions

> Implementation map — output format, architecture, action catalog, codegen, and the consumer side: see [implementation.md](./implementation.md).

## Summary

Rolldown devtools is a tracing-based system that emits structured build-time data (module graphs, chunk graphs, plugin hook calls, generated assets) to disk so that external tools (e.g. Vite devtools) can consume it to provide debugging, profiling, and visualization experiences.

## Lifecycle Principles

- Output identity follows the canonical filesystem identity of an existing `cwd`, not its lexical spelling. Symlink, `.`/`..`, drive-letter, and case aliases must converge without changing where that filesystem path resolves.
- The output files belong to a logical `(canonical output root, raw session ID)` pair, but every tracer has an independent owner lease. Same-key bundlers may append to the same files; one close or drop must not consume another owner's state or diagnostics.
- `$ref:` dictionaries are file-local. A consumer must be able to parse `meta.json` or `logs.json` independently without loading the other file first.
- Ordinary tracing writes are best-effort and must not unwind build work when the process-global writer cannot start or be accessed. The authoritative close path still returns structured failures.
- Global subscriber installation is serialized, fallible, and capability-aware. When `RD_LOG` is enabled first, normal tracing and the dormant devtools layers are installed together; an existing external subscriber becomes a build diagnostic instead of an N-API-crossing panic. Because tracing subscribers cannot gain layers after global installation, requesting `RD_LOG` after a devtools-only subscriber returns an explicit recoverable incompatibility instead of pretending logging was enabled.
- A devtools event is accepted only when its span ancestry carries both the selected session ID and canonical output root. Non-opted-in builds cannot fall back into another owner's files, and action callsites stay disabled while no tracer lease is active.

## Future Directions

### Plugin-Supplied Metadata

Consumers cannot always tell what a virtual module is or which package a plugin came from. Rolldown lets plugins attach descriptive metadata to both, so the devtools output can surface richer, human-authored context. The metadata is informational and does not affect bundling. See vitejs/devtools [#171](https://github.com/vitejs/devtools/issues/171) (module descriptions) and [#172](https://github.com/vitejs/devtools/issues/172) (plugin package name).

**Author-facing contract (already typed in `packages/rolldown`):**

- **Module description** — plugins return a top-level `description` from the `resolveId`/`load`/`transform` hooks. It is a direct optional field on `ModuleOptions` (so it is also readable via `this.getModuleInfo`) and works for dynamically-generated virtual modules.
- **Plugin metadata** — plugins set `meta.packageName`, `meta.version`, and `meta.description` on the plugin object (`PluginMeta`).

These fields are all optional and inert to bundling. They are general-purpose descriptive metadata; the tracing layer is one consumer that forwards them to tools such as Vite devtools.

**Emission side (not yet wired):** the trace actions need to carry these values so consumers actually receive them.

- `SessionMeta.plugins[]` (`PluginItem` in `rolldown_devtools_action`) should gain `package_name: Option<String>`, `version: Option<String>`, and `description: Option<String>`, populated by reading each plugin's `meta` when the session meta is emitted.
- Module descriptions should be attached where per-module data is emitted (e.g. `HookLoadCallEnd` / `ModuleGraphReady`), read from the module's resolved `description`.

Until that plumbing lands, setting the fields is a no-op beyond type-checking; the contract is defined first so plugin authors and consumers can align on the shape.

### Performance

The initial implementation prioritized unblocking consumability — getting structured data out to disk so that external tools could start building on it. Performance was explicitly not a priority at that stage.

Now that the system is in use, it's a major issue. On large projects, enabling devtools causes builds to become incredibly slow. The main bottlenecks:

- **Synchronous JSON serialization on the hot path.** Every `trace_action!` call serializes the action struct to JSON via `serde_json::to_string`, then the formatter parses it back into `serde_json::Value` for context injection and file writing. This double serialization happens inline during the build.
- **Full module content in hook events.** `HookLoadCallEnd`, `HookTransformCallStart/End`, and `HookRenderChunkStart/End` include the full source text of every module. For large codebases this means serializing and writing megabytes of source code per build.
- **blake3 hashing for dedup.** Every string >5 KB is hashed, and every string >10 KB triggers a hash lookup and `$ref` replacement. This adds CPU work proportional to total source size.
- **Per-event channel and writer work.** `DevtoolsFormatter::format_event` sends each shaped `serde_json::Value` to a process-global background writer. File creation, hashing, serialization to JSON lines, buffering, and flushing are off the build thread, but every event still incurs an allocation-heavy channel handoff and is processed individually.

Potential approaches:

- **Batch writer commands.** Accumulate multiple shaped actions per command or build boundary to reduce channel and per-event writer overhead while preserving the acknowledged close contract.
- **Lazy content emission.** Don't include full source in hook events by default. Instead, emit a content hash or offset reference; let the consumer request full content on demand (or write content to separate sidecar files).
- **Avoid double serialization.** Serialize directly to the output format instead of going through `serde_json::Value` as an intermediate.
- **Tiered verbosity.** Let users opt into levels of detail (e.g. graph-only vs. full hook tracing) so lightweight consumers don't pay for data they don't need.

### Storage Backend

The current storage model — appending JSON lines to a single `logs.json` file — does not scale. On large projects, a single build can produce ~3 GB of devtools data. At that size:

- **Consumers cannot load the file.** Parsing 3 GB of JSON into memory is impractical for a browser-based UI or even a Node.js process. The entire point of emitting data is for tools to consume it, and the current format makes that impossible at scale.
- **No random access.** To find a specific module's transform history, a consumer must scan the entire file linearly. There's no way to query "all HookTransformCall events for module X" without reading everything.
- **No incremental consumption.** In watch mode, the file grows across rebuilds with no structure to distinguish boundaries. A consumer that already processed build N has no efficient way to read only build N+1's events.

#### Database-Backed Storage

A real database backend would address all of these and unlock new capabilities:

**Local embedded DB (e.g. SQLite):**

- Structured tables per action type — consumers query only what they need
- Indexed by module ID, plugin name, build ID, timestamp — fast lookups without full scans
- WAL mode allows concurrent read/write — consumer can tail events while the build is running
- Single-file deployment, no external process needed
- Natural fit for the existing `node_modules/.rolldown/<session_id>/` layout (one `.db` file instead of `.json` files)

**Remote DB:**

- Unlocks centralized devtools for CI/CD — build data from CI pipelines flows to a shared store that developers can query from a dashboard
- Team-wide visibility into build performance regressions across commits
- Historical analysis — compare module graph evolution, plugin timing trends, chunk size growth over time
- Could be opt-in via `devtools: { backend: 'remote', endpoint: '...' }`

#### Schema Considerations

The action types already have well-defined structures (`SessionMeta`, `ModuleGraphReady`, `ChunkGraphReady`, etc.) that map naturally to relational tables. A sketch:

```
sessions(session_id, cwd, platform, format, dir, file, created_at)
builds(build_id, session_id, started_at, ended_at)
modules(build_id, module_id, is_external)
module_imports(build_id, module_id, imported_module_id, kind, module_request)
chunks(build_id, chunk_id, name, reason, is_user_defined_entry, is_async_entry, entry_module)
chunk_imports(build_id, chunk_id, imported_chunk_id, kind)
sources(source_id, content)  -- store large payloads/source text once
hook_calls(build_id, call_id, hook_type, plugin_name, plugin_id, module_id, started_at, ended_at, input_source_id, output_source_id)
assets(build_id, filename, chunk_id, size, content_source_id)
```

Separating large content from metadata means consumers querying plugin timing never touch the multi-GB source text. For a database-backed design specifically, source-like payloads should live in standalone fields/rows (for example, `sources.content`) and actions should reference them by ID (`input_source_id`, `output_source_id`, `content_source_id`) instead of inlining the same source everywhere. This is the relational equivalent of the current `StringRef` dedup pattern, but with proper query support.

#### Migration Path

The storage backend could be abstracted behind a trait so the formatter writes to a `DevtoolsWriter` instead of directly to files:

```rust
trait DevtoolsWriter: Send + Sync {
    fn write_action(&self, session_id: &str, build_id: &str, action: &serde_json::Value);
}
```

This allows the JSON-lines file writer to remain as the default (zero new dependencies) while a SQLite or remote backend can be plugged in via configuration. The `@rolldown/debug` consumer package would gain a corresponding `DevtoolsReader` abstraction.

### Per-Build Scoping (vs. Global Activation)

The current implementation uses one process-global `tracing_subscriber`
registry, but output ownership remains per bundler. `RD_LOG` initialization and
the first devtools initialization share one serialized installation result.
The installed devtools filters consult an active-tracer count before enabling
action callsites and all span callsites. The first acquisition and final release
rebuild tracing's process-wide callsite-interest cache, so callsites observed
before a traced build can transition from `never` to `always` without requiring
an active-count load on every action event. The filter intentionally leaves the
maximum-level hint unspecified: trace-level actions must remain eligible when a
co-installed `RD_LOG=info` layer has a lower maximum. The formatter rejects
events whose span ancestry lacks the devtools output-root marker. A build that
did not opt in therefore produces no devtools output even while another build
is traced.

The remaining global aspect is cost coupling: while any tracer is active,
devtools action callsites are enabled process-wide and unrelated builds may
still pay event-construction cost before the formatter rejects their missing
session marker. True per-future subscribers would remove that residual cost.

#### `tracing` Scoped Subscriber Mechanisms

The `tracing` crate provides several scoping primitives:

**`set_default` / `with_default`** — Sets a thread-local subscriber, returns a `DefaultGuard` that restores the prior subscriber on drop. **Thread-local only** — does **not** survive `.await` on multi-threaded tokio runtimes. When a task migrates to a different worker thread after an await point, it loses the scoped subscriber and falls back to the global default.

**`.with_subscriber()` (`WithDispatch`)** — The most promising primitive. Wraps an async future so the subscriber is re-installed into thread-local storage on **every poll**. This is async-safe: regardless of which thread polls the future, the correct subscriber is active.

Under the hood, `WithDispatch` implements `Future` by calling `set_default` before every `poll`:

```rust
// Simplified from tracing's instrument.rs
impl<T: Future> Future for WithDispatch<T> {
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _default = dispatcher::set_default(this.dispatcher); // set TLS on EVERY poll
        this.inner.poll(cx)
    }
}
```

Usage for per-bundler scoping would look like:

```rust
use tracing::Instrument; // also brings in WithSubscriber trait

let devtools_subscriber = tracing_subscriber::registry()
    .with(DevtoolsLayer)
    .with(fmt::layer().event_format(DevtoolsFormatter));

// Each bundler's top-level future gets its own subscriber
let build_future = bundle.write().with_subscriber(devtools_subscriber);
tokio::spawn(build_future);
```

**Key caveat: `tokio::spawn` does NOT inherit.** If the wrapped future internally calls `tokio::spawn(sub_task)`, the sub-task falls back to the global default subscriber. Every internal spawn must be explicitly wrapped:

```rust
// Inside bundler code that spawns sub-tasks:
let sub_task = do_work().with_current_subscriber(); // captures current thread-local subscriber
tokio::spawn(sub_task);
```

Missing a single `.with_current_subscriber()` silently drops the subscriber context for that task. This is the main risk for rolldown, which spawns tasks internally in the scan stage and elsewhere.

**Per-layer filtering on a global registry** — Keep one global subscriber installed via `.init()`, but attach per-layer `FilterFn`s that route events based on span fields (e.g. session ID). No propagation issue since the subscriber is global; the complexity shifts to the filter logic and dynamic layer management.

#### Applicability to Rolldown

| Approach                                     | Async-safe? | `tokio::spawn` propagation?                          | Complexity | Fits rolldown?                                                                                          |
| -------------------------------------------- | ----------- | ---------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------- |
| **`.with_subscriber()` per bundler future**  | **Yes**     | Manual (`.with_current_subscriber()` on every spawn) | Medium     | **Best semantic fit** — true per-bundler isolation. Requires auditing all internal `tokio::spawn` sites |
| Per-layer filtering on global registry       | Yes         | Free (global)                                        | Medium     | Good fit — session ID already in span context, no spawn propagation needed                              |
| `set_default` + `current_thread` per bundler | Yes         | Free (single-thread)                                 | High       | Impractical — changes the runtime model                                                                 |
| Session-aware check in `trace_action!`       | Yes         | N/A (pre-emit)                                       | Low        | Complementary — zero cost for disabled sessions regardless of which approach above is chosen            |

**`.with_subscriber()` is the strongest candidate** for true per-build isolation — it gives each bundler instance its own subscriber with clean separation. The `tokio::spawn` propagation gap is the main adoption cost: it requires auditing every internal spawn site and wrapping with `.with_current_subscriber()`. However, this is a one-time audit that also makes subscriber scoping correctness explicit in the codebase. A lint or wrapper helper (e.g. a `devtools_spawn(future)` that auto-wraps) could enforce this going forward.

Regardless of which approach is chosen for subscriber scoping, a **pre-emit check in `trace_action!`** should be added as a complementary optimization so disabled sessions skip serialization entirely.

## Unresolved Questions

- **Incremental/watch mode:** The devtools system works for both `ClassicBundler` (one-shot) and core `Bundler` (incremental), but successive builds within the same session append to the same `logs.json`. No explicit "rebuild boundary" action exists yet.
- **Dev engine integration:** `BindingDevEngine` creates a session but uses `Session::dummy()` — devtools is not yet wired up for the dev/HMR engine.

## Related

- [implementation.md](./implementation.md) — the devtools implementation map
- [rust-classic-bundler](../rust-classic-bundler/implementation.md) — ClassicBundler design, references devtools session/tracer fields
- [rust-bundler](../rust-bundler/implementation.md) — Core Bundler design, references session field
