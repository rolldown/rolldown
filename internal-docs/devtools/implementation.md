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

The `devtools` option is `@experimental`. Setting `devtools: {}` is sufficient to enable tracing. The option flows through the binding layer as `BindingDevtoolsOptions` and normalizes to `DevtoolsOptions { session_id: Option<String> }` on the Rust side. `ClassicBundler` keeps its constructor-generated ID when `sessionId` is omitted, or replaces it with the requested value before creating the tracer and session span. That selected ID is therefore shared by emitted event fields, output-directory naming, and the acknowledged close flush.

CLI equivalent: `--devtools.session-id <id>`.

## Output

When devtools is enabled, rolldown writes JSON-lines files to:

```
<InputOptions.cwd>/node_modules/.rolldown/<safe_session_component>/
  meta.json    # SessionMeta action (one JSON object per build; appended in watch/rebuild)
  logs.json    # All other actions, one JSON object per line
```

The JavaScript API normalizes an omitted `cwd` to `process.cwd()` on Node and
to `/` in browser builds. `ClassicBundler` canonicalizes an existing cwd before
appending `node_modules/.rolldown` and records that root on the session span.
Symlink, `.`/`..`, case, and drive aliases therefore share one filesystem
identity without canonicalizing or relocating the output directory itself. If
the cwd cannot be canonicalized, the writer falls back to absolute lexical
normalization so the original I/O error is still reported through close.

The raw `sessionId` remains the `session_id` value in emitted actions. For the
directory name, portable non-empty IDs containing only lowercase ASCII letters,
digits, `-`, and `_` are preserved (up to 200 bytes, excluding Windows device names).
Other IDs are encoded as one `~`-prefixed lowercase-hex component, or as a
fixed-size `~h<blake3>` component when the hex form would be too long. Path
separators, `.`/`..`, absolute paths, Unicode, and platform-reserved names
therefore cannot change the output root. Encoding uppercase IDs also prevents
distinct raw IDs from aliasing on case-insensitive filesystems.

Each line is a self-contained JSON object with an `action` discriminator field. Action events also carry `timestamp`, `session_id`, and `build_id` fields. `StringRef` entries contain only `action`, `id`, and `content` (no timestamp). The consumer reads the file and splits on newlines.

### Read-after-close contract

`meta.json` and `logs.json` are only guaranteed to be complete and readable **after `await bundle.close()` resolves successfully**. Native and threaded-WASI events flow through a channel to a background writer thread; genuine threadless WASI processes the same commands synchronously behind a mutex because it cannot create OS threads. Both backends buffer file output via `BufWriter`, so reading the files immediately after `generate()`/`write()` may return empty or truncated content. `bundle.close()` sends a per-owner `CloseSession` command with an ack channel and awaits the writer result, establishing the happens-before edge consumers depend on. Commands are processed serially in submission order, so the close result covers every earlier write command for that logical session.

The writer retains directory creation, file open, event serialization/write, and flush failures per logical session and clones each distinct failure into every active owner's queue. A newly overlapping owner inherits already-retained failures. Repeated failures for the same operation/path are coalesced to the first diagnostic while the writer still retries the I/O, preventing one broken directory from creating an unbounded close aggregate. Every owner close flushes the shared files and returns only that owner's queue. Shared files, dictionaries, and retained failures remain available until the final owner closes; duplicate authoritative/fallback closes are no-ops. Consequently, one same-root/same-ID bundler cannot consume another's state or diagnostics.

Writer startup, global backend access, command submission, ordinary command-processing panics, and per-file close flush panics are contained. A flush panic becomes an owner-scoped `FlushFile` failure while final-owner state is still retired, so the writer thread can acknowledge the close and continue serving other sessions. Ordinary formatter writes and tracer-drop cleanup remain best-effort; an authoritative close receives a structured failure if the backend could not start, was poisoned, or disconnected. The internal `closeTerminal()` transport reports each distinct writer failure as a separate binding diagnostic. Direct `BindingBundler.close()` consumers receive a JavaScript `AggregateError` when multiple failures exist; original JavaScript errors retain their object identity inside its `errors` array, while a lone JavaScript failure remains the rejection object itself.

The process-global tracing subscriber has one serialized retained
initialization result and records whether normal logging was installed.
`RD_LOG` installs the normal logging layer and dormant devtools layers together,
so enabling both facilities cannot attempt a second global subscriber
installation. The first devtools-only build uses the same initializer when
normal tracing is disabled. A later `RD_LOG` request cannot mutate that global
subscriber, so it reports an explicit incompatibility on stderr instead of
silently returning success without logging. Initialization constructs and
retains the installed `tracing::Dispatch`; an external subscriber conflict or
contained initializer panic becomes a bundler-initialization diagnostic instead
of aborting Node.

### Large String Deduplication

Top-level string fields larger than 5 KB are cached by blake3 hash independently
for each output file. A `StringRef` record is emitted as its own JSON line before
the action that references it:

```json
{ "action": "StringRef", "id": "<blake3-hash>", "content": "<full string>" }
```

Top-level string fields larger than 10 KB are additionally replaced with a `$ref:<hash>` placeholder in the action itself, pointing back to a `StringRef` entry in the same file. This keeps action records compact while allowing `meta.json` and `logs.json` to be consumed independently. Note: nested strings (e.g. `AssetsReady.assets[].content`) are not ref'd — only top-level fields are considered.

Structural routing fields (`action`, `build_id`, and `session_id`) are never
dictionary entries and are never replaced. In particular, emitted
`session_id` always remains the exact raw configured value even when the
filesystem directory uses a bounded encoded component.

## Architecture

### Crate Layout

| Crate                      | Purpose                                                                    |
| -------------------------- | -------------------------------------------------------------------------- |
| `rolldown_devtools`        | Core tracing machinery: `DebugTracer`, `Session`, formatter, layer         |
| `rolldown_devtools_action` | Action type definitions (Rust structs with `ts-rs` for TS codegen)         |
| `@rolldown/debug`          | TypeScript package: re-exports generated types + `parseToEvents()` utility |

### Key Types

- **`DebugTracer`** — Acquires the serialized process-global subscriber result, registers one writer owner, and increments the active-tracer count used by both devtools layer filters. Its `DevtoolsSessionKey` combines the logical canonical-root/raw-ID pair with a unique owner ID. Clones share one `Arc` lease, so only the final clone sends the best-effort no-ack close fallback and decrements the active count. Each admitted native operation guard retains a clone, preventing N-API object finalization from closing the owner while that operation can still emit events. The authoritative flush path clones the same owner key into `ClassicBundler::close()`, passes it to `rolldown_devtools::flush_session(...)`, and awaits the structured result before resolving.
- **`Session`** — Holds a session `id` (e.g. `sid_0_1710000000000`) and a parent `tracing::Span`. All build spans are children of the session span. A `Session::dummy()` is used when devtools is disabled (no-op span).
- **`DevtoolsLayer`** — A `tracing_subscriber::Layer` that extracts `CONTEXT_*` prefixed fields from spans and stores them as `ContextData` in span extensions.
- **`DevtoolsFormatter`** — A `FormatEvent` impl that parses `devtoolsAction`-tagged events, injects context variables, consumes the output root carried by the session span, encodes the session directory component, and submits the resolved action to the selected writer backend.

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

**Event filtering:** Both `rolldown_devtools` and `rolldown_tracing` use the same devtools filter, which accepts action events carrying `devtoolsAction` and all spans. The layer stores extensions only for spans declaring `CONTEXT_*` fields, but the formatter must observe intermediate spans as well so an explicitly parented non-context span cannot sever its path to session ancestry. When `RD_LOG` is active, its normal layer is installed inside the devtools layers; otherwise the normal layer's rejecting filter IDs would be inherited by the formatter context and hide devtools span ancestry. The devtools filters expose cached `never` interest while no tracer lease exists and cached `always` interest while one is active. They intentionally return no maximum-level hint, so a co-installed lower-verbosity normal layer cannot globally suppress trace-level devtools actions. The `0 -> 1` and `1 -> 0` tracer transitions rebuild tracing's callsite-interest cache under the retained installed dispatch; this is required because N-API operations may perform the transition on a worker with a different thread-local default. Callsites first observed by an untraced build can therefore become active later without an active-count load on every steady-state action. The normal tracing layer (chrome/console) filters devtools actions _out_, so they do not pollute standard trace output. The formatter additionally requires `session_id` and `devtools_output_root` from span ancestry before routing a command. Missing context is discarded rather than mapped to a fallback owner.

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

The process-global writer backend owns one `WriterState`. Native and
`wasm32-wasip1-threads` builds use a channel plus one background writer thread.
The genuine threadless `wasm32-wasip1` build stores the same state behind an
inline mutex and processes commands synchronously:

- `files` — one buffered file handle and `StringRef` dictionary per output path
- `files_by_session` — files belonging to each logical session
- `dir_ensured` — logical sessions whose output-directory creation has succeeded
- `owners_by_session` — active unique owner leases for each logical session
- `failures_by_session` — retained distinct failures used to seed overlapping owners
- `failures_by_owner` — independent close result queue for each active owner

Formatter writes carry the logical key made from the canonical
`<cwd>/node_modules/.rolldown` output root plus raw session ID. Registration and
close commands carry the public key, which adds a unique owner ID. Reusing an
ID in different cwd values cannot merge state, while same-root/same-ID owners
intentionally append to the same files without sharing close ownership.

Each backend serializes access to this state, so write, register, and close commands cannot interleave inside a file operation. Writes with no active owner are ignored, preventing late events from reopening a finalized session. `CloseSession` flushes the addressed logical session, unregisters exactly one owner, and only removes shared state for the final owner. The ack returns `Result<(), DevtoolsWriterError>` containing that owner's retained failures. When the threaded backend's process-global channel disconnects, the writer flushes and clears any remaining state best-effort.

## Consumer Side

The `@rolldown/debug` package provides:

```ts
import { parseToEvents, type Event, type StringRef } from '@rolldown/debug';

const data = fs.readFileSync('node_modules/.rolldown/<safe-session-component>/logs.json', 'utf8');
const events = parseToEvents(data.trim());
// events: Array<StringRef | { timestamp, session_id, action: "BuildStart" | "ModuleGraphReady" | "PackageGraphReady" | ... }>
```

Consumers (like Vite devtools) read the JSON-lines files, resolve `$ref:<hash>` placeholders against `StringRef` entries, and reconstruct the full build timeline.

## Related

- [design.md](./design.md) — devtools future directions and open questions
- [rust-classic-bundler](../rust-classic-bundler/implementation.md) — ClassicBundler design, references devtools session/tracer fields
- [rust-bundler](../rust-bundler/implementation.md) — Core Bundler design, references session field
