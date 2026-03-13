# Devtools

## Summary

Rolldown devtools is a tracing-based system that emits structured build-time data (module graphs, chunk graphs, plugin hook calls, generated assets) to disk so that external tools (e.g. Vite devtools) can consume it to provide debugging, profiling, and visualization experiences.

## User-Facing API

```ts
import { rolldown } from 'rolldown';

const bundle = await rolldown({
  input: 'src/index.js',
  devtools: {
    sessionId?: string,  // optional override; auto-generated if omitted
  },
});
await bundle.generate();
```

The `devtools` option is `@experimental`. Setting `devtools: {}` is sufficient to enable tracing. The option flows through the binding layer as `BindingDevtoolsOptions` and normalizes to `DevtoolsOptions { session_id: Option<String> }` on the Rust side.

CLI equivalent: `--devtools.session-id <id>`.

## Output

When devtools is enabled, rolldown writes JSON-lines files to:

```
<CWD>/node_modules/.rolldown/<session_id>/
  meta.json    # SessionMeta action (one JSON object per build; appended in watch/rebuild)
  logs.json    # All other actions, one JSON object per line
```

Each line is a self-contained JSON object with an `action` discriminator field. Action events also carry `timestamp`, `session_id`, and `build_id` fields. `StringRef` entries contain only `action`, `id`, and `content` (no timestamp). The consumer reads the file and splits on newlines.

### Large String Deduplication

Strings larger than 5 KB are deduplicated by blake3 hash. A `StringRef` record is emitted before the action that references it:

```json
{ "action": "StringRef", "id": "<blake3-hash>", "content": "<full string>" }
```

Strings larger than 10 KB in the action itself are replaced with a `$ref:<hash>` placeholder, pointing back to the `StringRef` entry. This keeps action records compact while preserving full content for consumers that need it.

## Architecture

### Crate Layout

| Crate                      | Purpose                                                                    |
| -------------------------- | -------------------------------------------------------------------------- |
| `rolldown_devtools`        | Core tracing machinery: `DebugTracer`, `Session`, formatter, layer         |
| `rolldown_devtools_action` | Action type definitions (Rust structs with `ts-rs` for TS codegen)         |
| `@rolldown/debug`          | TypeScript package: re-exports generated types + `parseToEvents()` utility |

### Key Types

- **`DebugTracer`** — Initializes a `tracing_subscriber` registry with the devtools-specific layer and formatter. Singleton init via `AtomicBool`. On drop, cleans up file handles and hash caches for its session.
- **`Session`** — Holds a session `id` (e.g. `sid_0_1710000000000`) and a parent `tracing::Span`. All build spans are children of the session span. A `Session::dummy()` is used when devtools is disabled (no-op span).
- **`DevtoolsLayer`** — A `tracing_subscriber::Layer` that extracts `CONTEXT_*` prefixed fields from spans and stores them as `ContextData` in span extensions.
- **`DevtoolsFormatter`** — A `FormatEvent` impl that serializes `devtoolsAction`-tagged events to JSON lines, injects context variables, and writes to the appropriate file.

### Tracing Mechanism

The system is built on the `tracing` crate. The core idea: **spans carry context implicitly, events carry data explicitly**.

```
<SessionSpan CONTEXT_session_id="sid_0_...">
  <BuildSpan CONTEXT_build_id="bid_0_count_0" CONTEXT_hook_resolve_id_trigger="automatic">
    {trace_action!(BuildStart { action: "BuildStart" })}
    <HookResolveIdCallSpan CONTEXT_call_id="uuid-v4">
      {trace_action!(HookResolveIdCallStart { ..., trigger: "${hook_resolve_id_trigger}", call_id: "${call_id}" })}
      ...
      {trace_action!(HookResolveIdCallEnd { ... })}
    </HookResolveIdCallSpan>
    {trace_action!(ModuleGraphReady { ... })}
    {trace_action!(ChunkGraphReady { ... })}
    {trace_action!(BuildEnd { action: "BuildEnd" })}
  </BuildSpan>
</SessionSpan>
```

**Why spans?**

- Context injection without manual plumbing — `session_id`, `build_id`, `call_id` are all resolved from ancestor spans at emit time via `${variable_name}` placeholder substitution.
- Automatic async context tracking — spans follow across `.await` boundaries.

**Event filtering:** Both `rolldown_devtools` and `rolldown_tracing` filter events by the presence of the `devtoolsAction` field. The devtools layer only processes events with that field; the normal tracing layer (chrome/console) filters them _out_, so devtools events don't pollute standard trace output.

### ID Generation

- **Session ID:** `sid_{atomic_seed}_{unix_ms}` — unique per `ClassicBundler` / `Bundler` instance.
- **Build ID:** `bid_{atomic_seed}_count_{build_count}` — unique per `Bundle` within a session. The `build_count` increments per build in the same `BundleFactory`.

### Lifecycle Integration

**`ClassicBundler`** (binding layer, Rollup-compatible API):

1. `new()` — generates `session_id`, creates dummy session
2. `enable_debug_tracing_if_needed()` — on first build with `devtools` option, initializes `DebugTracer` and creates real session span
3. Passes `Session` to `BundleFactory` on each `create_bundle()` call

**`BundleFactory`** (core):

1. Stores session, generates unique build spans via `generate_unique_bundle_span()`
2. Each span is a child of `session.span` with `CONTEXT_build_id` and `CONTEXT_hook_resolve_id_trigger` fields

**`Bundle`** (per-build):

1. `trace_action_session_meta()` — emits `SessionMeta` with inputs, plugins, cwd, platform, format, output dir/file
2. `BuildStart` / `BuildEnd` — bracket the full build
3. `trace_action_module_graph_ready()` — emits after scan stage with all modules and their import relationships
4. `trace_action_chunks_infos()` — emits after chunk graph construction in the generate stage

**`PluginDriver`** (plugin hooks):

- `resolve_id` — `HookResolveIdCallStart` / `HookResolveIdCallEnd` wrapped in a `HookResolveIdCall` span with `CONTEXT_call_id`
- `load` — `HookLoadCallStart` / `HookLoadCallEnd` wrapped similarly
- `transform` — `HookTransformCallStart` / `HookTransformCallEnd`
- `render_chunk` — `HookRenderChunkStart` / `HookRenderChunkEnd`

Each hook call pair gets a unique `call_id` (UUID v4) via its enclosing span.

## Action Catalog

| Action                       | When Emitted                         | Key Fields                                                                                                  |
| ---------------------------- | ------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| `SessionMeta`                | Start of build (to `meta.json`)      | inputs, plugins, cwd, platform, format, dir, file                                                           |
| `BuildStart`                 | Before scan stage                    | —                                                                                                           |
| `HookResolveIdCallStart/End` | Per plugin per resolve call          | module_request, importer, plugin_name, plugin_id, trigger, call_id, resolved_id                             |
| `HookLoadCallStart/End`      | Per plugin per load call             | module_id, plugin_name, plugin_id, call_id, content                                                         |
| `HookTransformCallStart/End` | Per plugin per transform call        | module_id, content, plugin_name, plugin_id, call_id                                                         |
| `ModuleGraphReady`           | After scan + normalize               | modules[]{id, is_external, imports[]{module_id, kind, module_request}, importers[]}                         |
| `BuildEnd`                   | After scan stage completes           | —                                                                                                           |
| `ChunkGraphReady`            | After chunk graph construction       | chunks[]{chunk_id, name, reason, modules[], imports[], is_user_defined_entry, is_async_entry, entry_module} |
| `HookRenderChunkStart/End`   | Per plugin per renderChunk call      | chunk_id, plugin_name, plugin_id, call_id, code                                                             |
| `AssetsReady`                | After final asset generation         | assets[]{chunk_id, content, size, filename}                                                                 |
| `StringRef`                  | Before any action with large strings | id (blake3 hash), content                                                                                   |

All actions except `StringRef` carry injected `session_id`, `build_id`, and `timestamp` fields. `StringRef` entries contain only `action`, `id`, and `content`.

## TypeScript Codegen

Action types are defined as Rust structs with `#[derive(ts_rs::TS, serde::Serialize)]`. The codegen pipeline:

1. `cargo test -p rolldown_devtools_action export_bindings` — ts-rs generates `.ts` files in `crates/rolldown_devtools_action/bindings/`
2. `scripts/src/gen-debug-action-types.ts` — copies to `packages/debug/src/generated/`, creates barrel `index.ts`
3. `packages/debug` publishes as `@rolldown/debug` — exports all action types plus `parseToEvents()` / `parseToEvent()` utilities

Run: `pnpm --filter @rolldown/debug run gen-action-types`

## Static Data Management

File handles and hash caches are stored in process-global `LazyLock<DashMap>` statics:

- `OPENED_FILE_HANDLES` — one file handle per output file path, preventing duplicate writes
- `OPENED_FILES_BY_SESSION` — tracks which files belong to which session (for cleanup)
- `EXIST_HASH_BY_SESSION` — tracks already-emitted `StringRef` hashes per session (for dedup)

These are cleaned up when `DebugTracer` is dropped.

## Consumer Side

The `@rolldown/debug` package provides:

```ts
import { parseToEvents, type Event, type StringRef } from '@rolldown/debug';

const data = fs.readFileSync('node_modules/.rolldown/<sid>/logs.json', 'utf8');
const events = parseToEvents(data.trim());
// events: Array<StringRef | { timestamp, session_id, action: "BuildStart" | "ModuleGraphReady" | ... }>
```

Consumers (like Vite devtools) read the JSON-lines files, resolve `$ref:<hash>` placeholders against `StringRef` entries, and reconstruct the full build timeline.

## Future Directions

### Performance

The initial implementation prioritized unblocking consumability — getting structured data out to disk so that external tools could start building on it. Performance was explicitly not a priority at that stage.

Now that the system is in use, it's a major issue. On large projects, enabling devtools causes builds to become incredibly slow. The main bottlenecks:

- **Synchronous JSON serialization on the hot path.** Every `trace_action!` call serializes the action struct to JSON via `serde_json::to_string`, then the formatter parses it back into `serde_json::Value` for context injection and file writing. This double serialization happens inline during the build.
- **Full module content in hook events.** `HookLoadCallEnd`, `HookTransformCallStart/End`, and `HookRenderChunkStart/End` include the full source text of every module. For large codebases this means serializing and writing megabytes of source code per build.
- **blake3 hashing for dedup.** Every string >5 KB is hashed, and every string >10 KB triggers a hash lookup and `$ref` replacement. This adds CPU work proportional to total source size.
- **Synchronous file I/O.** `DevtoolsFormatter::format_event` writes directly to files via `std::fs::File`, blocking the thread.

Potential approaches:

- **Async/buffered writes.** Move file I/O off the build thread — buffer events in memory and flush on a background thread or at build boundaries.
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
hook_calls(build_id, call_id, hook_type, plugin_name, plugin_id, module_id, started_at, ended_at)
hook_call_content(call_id, content_before, content_after)  -- large text in separate table
assets(build_id, filename, chunk_id, size)
```

Separating large content (`hook_call_content`) from metadata (`hook_calls`) means consumers querying plugin timing never touch the multi-GB source text. This is the relational equivalent of the current `StringRef` dedup pattern, but with proper query support.

#### Migration Path

The storage backend could be abstracted behind a trait so the formatter writes to a `DevtoolsWriter` instead of directly to files:

```rust
trait DevtoolsWriter: Send + Sync {
    fn write_action(&self, session_id: &str, build_id: &str, action: &serde_json::Value);
}
```

This allows the JSON-lines file writer to remain as the default (zero new dependencies) while a SQLite or remote backend can be plugged in via configuration. The `@rolldown/debug` consumer package would gain a corresponding `DevtoolsReader` abstraction.

### Per-Build Scoping (vs. Global Activation)

The current implementation uses a process-global `tracing_subscriber` registry initialized via `DebugTracer::init()` with an `AtomicBool` singleton guard. This means:

- Setting `devtools: {}` in **one** rolldown config causes **all** bundler instances in the same process to emit devtools data, even those that didn't opt in.
- There's no way to enable devtools for one build and disable it for another within the same process (e.g. a monorepo tool running multiple rolldown builds).

The root cause is that `tracing_subscriber::registry().init()` installs a global subscriber. Once installed, every `tracing::trace!` event in the process flows through the devtools layer.

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

- **Output location:** Currently hardcoded to `node_modules/.rolldown/` relative to real `process.cwd()`, not `InputOptions.cwd`. This means the devtools output may not land where expected if cwd differs.
- **Incremental/watch mode:** The devtools system works for both `ClassicBundler` (one-shot) and core `Bundler` (incremental), but successive builds within the same session append to the same `logs.json`. No explicit "rebuild boundary" action exists yet.
- **Dev engine integration:** `BindingDevEngine` creates a session but uses `Session::dummy()` — devtools is not yet wired up for the dev/HMR engine.

## Related

- [rust-classic-bundler](./rust-classic-bundler.md) — ClassicBundler design, references devtools session/tracer fields
- [rust-bundler](./rust-bundler.md) — Core Bundler design, references session field
