# Devtools — Implementation

> Forward-looking design (future directions) and open questions live in [design.md](./design.md).

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

### Read-after-close contract

`meta.json` and `logs.json` are only guaranteed to be complete and readable **after `await bundle.close()` resolves**. Internally, events flow through a channel to a background writer thread and are buffered via `BufWriter`, so reading the files immediately after `generate()`/`write()` may return empty or truncated content. `bundle.close()` sends a `CloseSession` command with an ack channel and awaits the writer thread's signal, establishing the happens-before edge consumers depend on.

### Large String Deduplication

Top-level string fields larger than 5 KB are cached by blake3 hash. A `StringRef` record is emitted before the action that references it:

```json
{ "action": "StringRef", "id": "<blake3-hash>", "content": "<full string>" }
```

Top-level string fields larger than 10 KB are additionally replaced with a `$ref:<hash>` placeholder in the action itself, pointing back to the `StringRef` entry. This keeps action records compact while preserving full content for consumers that need it. Note: nested strings (e.g. `AssetsReady.assets[].content`) are not ref'd — only top-level fields are considered.

## Architecture

### Crate Layout

| Crate                      | Purpose                                                                    |
| -------------------------- | -------------------------------------------------------------------------- |
| `rolldown_devtools`        | Core tracing machinery: `DebugTracer`, `Session`, formatter, layer         |
| `rolldown_devtools_action` | Action type definitions (Rust structs with `ts-rs` for TS codegen)         |
| `@rolldown/debug`          | TypeScript package: re-exports generated types + `parseToEvents()` utility |

### Key Types

- **`DebugTracer`** — Initializes a `tracing_subscriber` registry with the devtools-specific layer and formatter. Singleton init via `AtomicBool`. On drop, sends a best-effort (no-ack) `CloseSession` to the writer thread as a cleanup fallback; the authoritative flush path is `ClassicBundler::close()`, which uses `rolldown_devtools::flush_session(session_id)` and awaits an ack before resolving.
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
    {trace_action!(PackageGraphReady { ... })}
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
2. `BuildStart` / `BuildEnd` — emitted both around the outer `write()`/`generate()` call and inside `scan_modules()`, so consumers may see nested pairs per build
3. `trace_action_module_graph_ready()` — emits after scan stage with all modules and their import relationships
4. `trace_action_chunks_infos()` — emits after chunk graph construction in the generate stage
5. `trace_action_package_graph_ready()` — emits after chunk instantiation with package metadata discovered from resolved package.json files

**`PluginDriver`** (plugin hooks):

- `resolve_id` — `HookResolveIdCallStart` / `HookResolveIdCallEnd` wrapped in a `HookResolveIdCall` span with `CONTEXT_call_id`
- `load` — `HookLoadCallStart` / `HookLoadCallEnd` wrapped similarly
- `transform` — `HookTransformCallStart` / `HookTransformCallEnd`
- `render_chunk` — `HookRenderChunkStart` / `HookRenderChunkEnd`

Each hook call pair gets a unique `call_id` (UUID v4) via its enclosing span.

## Action Catalog

| Action                       | When Emitted                              | Key Fields                                                                                                                     |
| ---------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `SessionMeta`                | Start of build (to `meta.json`)           | inputs, plugins, cwd, platform, format, dir, file                                                                              |
| `BuildStart`                 | Before scan stage + around write/generate | —                                                                                                                              |
| `HookResolveIdCallStart/End` | Per plugin per resolve call               | module_request, importer, plugin_name, plugin_id, trigger, call_id, resolved_id                                                |
| `HookLoadCallStart/End`      | Per plugin per load call                  | module_id, plugin_name, plugin_id, call_id, content                                                                            |
| `HookTransformCallStart/End` | Per plugin per transform call             | module_id, content, plugin_name, plugin_id, call_id                                                                            |
| `ModuleGraphReady`           | After scan + normalize                    | modules[]{id, is_external, imports[]{module_id, kind, module_request}, importers[]}                                            |
| `BuildEnd`                   | After scan stage + after write/generate   | —                                                                                                                              |
| `ChunkGraphReady`            | After chunk graph construction            | chunks[]{chunk_id, name, reason, modules[], imports[], is_user_defined_entry, is_async_entry, entry_module}                    |
| `PackageGraphReady`          | After chunk instantiation                 | packages[]{package_id, name, version, package_json_path, package_root, is_used, dependency_type, size, modules[], chunk_ids[]} |
| `HookRenderChunkStart/End`   | Per plugin per renderChunk call           | chunk_id, plugin_name, plugin_id, call_id, content                                                                             |
| `AssetsReady`                | After final asset generation              | assets[]{chunk_id, content, size, filename}                                                                                    |
| `StringRef`                  | Before any action with large strings      | id (blake3 hash), content                                                                                                      |

All actions except `StringRef` carry injected `session_id`, `build_id`, and `timestamp` fields. `StringRef` entries contain only `action`, `id`, and `content`.

`PackageGraphReady.packages` contains packages discovered from resolved module `package.json` files. `is_used` is true when at least one module for that package appears in a generated chunk, and false when all resolved modules for that package are tree-shaken. `dependency_type` is `direct` when any module in the package is imported by a source module under the build `cwd` and outside `node_modules`; otherwise it is `transitive`. This uses the importer graph and does not inspect `package.json` dependency fields. `size` is the sum of the package's rendered module code bytes after tree-shaking/codegen and before chunk-level `renderChunk`, minification, banners, and final asset emission. `modules` contains the package's generated chunk module IDs, and `chunk_ids` contains the matching `ChunkGraphReady` chunk IDs; both arrays are empty for unused packages. The packages are sorted by package name, version, package root, and package id. Rolldown does not emit a duplicate flag; consumers can identify duplicate packages by grouping non-null package names and checking whether a group contains multiple versions or package roots.

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

These are cleaned up when the background writer thread processes a `CloseSession` command — either sent synchronously via `flush_session(...)` from `ClassicBundler::close()` (ack-based, happens-before `close()` resolving) or best-effort from `DebugTracer::drop`.

## Consumer Side

The `@rolldown/debug` package provides:

```ts
import { parseToEvents, type Event, type StringRef } from '@rolldown/debug';

const data = fs.readFileSync('node_modules/.rolldown/<sid>/logs.json', 'utf8');
const events = parseToEvents(data.trim());
// events: Array<StringRef | { timestamp, session_id, action: "BuildStart" | "ModuleGraphReady" | "PackageGraphReady" | ... }>
```

Consumers (like Vite devtools) read the JSON-lines files, resolve `$ref:<hash>` placeholders against `StringRef` entries, and reconstruct the full build timeline.

## Related

- [design.md](./design.md) — devtools future directions and open questions
- [rust-classic-bundler](../rust-classic-bundler/implementation.md) — ClassicBundler design, references devtools session/tracer fields
- [rust-bundler](../rust-bundler/implementation.md) — Core Bundler design, references session field
