# Module Conversion — Design & Principles

## Summary

**Module conversion** is the step that turns a module's native content into
JavaScript, according to its `ModuleType`. For `js` it is a no-op; for
`ts`/`tsx`/`jsx` it is the oxc transformer; for `json`/`text`/`base64`/
`dataurl`/`binary`/`empty` it is a source rewrite; for `asset`/`copy` it emits
a file and yields a reference to it. These are all the same kind of operation
— `ModuleType` → JS — and today only some of them are treated that way.

This document defines module conversion as a single named pipeline stage with a
single observable contract:

> **`ModuleInfo.content` is the module's content as it exists after the
> `transform` hook and before module conversion.**

TypeScript already satisfies this: `this.load()` on a `.ts` module hands back
TypeScript source, not the lowered JavaScript, because lowering happens after
`transform`. Every other module type should behave the same way. That is the
whole of the proposal; everything below is what it costs to get there.

Naming: the docs currently call this stage `internalTransform`
(`docs/apis/plugin-api.md:130,141-142,151,248`). That name describes the
TypeScript/JSX half and hides the rest. It should be renamed to
**module conversion**.

All file/line references are against the working tree at the time of writing
and will drift; treat them as starting points.

## Target behavior

What `this.load({ id })` should return, per module type:

| `moduleType` | `content`                                      |
| ------------ | ---------------------------------------------- |
| `js`         | `export default "foo"`                         |
| `jsx`        | `export default <Foo />`                       |
| `ts`         | `export default "foo" satisfies string`        |
| `tsx`        | `export default <Foo /> satisfies JSX.Element` |
| `json`       | `{ "foo": "bar" }`                             |
| `text`       | `foo`                                          |
| `base64`     | binary content (not base64-encoded)            |
| `dataurl`    | binary content (not encoded as a data URL)     |
| `binary`     | binary content                                 |
| `empty`      | binary content                                 |
| `css`        | `.foo { color: red }`                          |
| `asset`      | binary content                                 |
| `copy`       | binary content                                 |

Read down the column: it is always "what is in the file, after plugins have had
their say." No placeholders, no encodings, no generated wrappers.

## What is wrong today

Conversion currently lives in three places, at three different points in the
pipeline, and `this.load` returns whatever each place happened to leave behind.

**1. `pre_process_source`** — `crates/rolldown/src/utils/parse_to_ecma_ast.rs:141-216`,
plus the `Json` arm of its caller at `:83-104`. This is the correct home. It
handles `text`/`base64`/`dataurl`/`binary`/`empty`, and it runs after
`transform`, in the same stage as TypeScript/JSX lowering.

**2. `AssetModulePlugin::load`** — `crates/rolldown_plugin_asset_module/src/lib.rs:126-189`.
Runs at `load`, i.e. _before_ `transform`. It reads the file, moves the bytes
into `EmittedAsset`, and returns `module.exports = "__ROLLDOWN_ASSET__#<ref>"`
with `module_type: Some(ModuleType::Js)`. By the time anything downstream looks
at the module, the bytes are gone and it no longer knows it was ever an asset.

**3. `CopyModulePlugin::resolve_id`** — `crates/rolldown_plugin_copy_module/src/lib.rs:51-131`.
Runs even earlier. It reads the file, emits it, and returns a prefixed id marked
`external: true`. The module never enters the graph at all.

The observable result:

| `moduleType`          | `this.load` returns today                                                            |     |
| --------------------- | ------------------------------------------------------------------------------------ | --- |
| `js`/`jsx`/`ts`/`tsx` | the source                                                                           | ✅  |
| `json`                | `""`                                                                                 | ❌  |
| `text`                | `"foo"` (JSON-escaped, quoted)                                                       | ❌  |
| `base64`              | `"aGVsbG8="`                                                                         | ❌  |
| `dataurl`             | `"data:text/plain;base64,aGVsbG8="`                                                  | ❌  |
| `binary`              | `import {__toBinary} from 'rolldown:runtime'; export default __toBinary('aGVsbG8=')` | ❌  |
| `empty`               | `""`                                                                                 | ❌  |
| `css`                 | hard error before parse (`module_task.rs:117-124`)                                   | ❌  |
| `asset`               | `module.exports = "__ROLLDOWN_ASSET__#ref-0"`                                        | ❌  |
| `copy`                | hard error: "Encountered a module with type `copy`, but no plugin handled it"        | ❌  |

Three of these deserve comment.

**`json` returns the empty string.** `json_value_to_ecma_ast` constructs its AST
with `ArcStr::from("")` as the source
(`crates/rolldown_common/src/ecmascript/json_to_program.rs:30`), and
`ecma_view.source` is `ast.source()`. So every JSON module in the graph reports
`code: ""`. This is a latent bug that the invariant fixes as a side effect.

**`copy` errors rather than returning a placeholder.** `this.load` bypasses
`resolveId` entirely — it constructs a `ResolvedId` directly from the specifier
(`crates/rolldown_plugin/src/plugin_context/native_plugin_context.rs:64-71`) —
so `CopyModulePlugin::resolve_id` never runs, and the core loader reaches the
`ModuleType::Copy` arm of `load_source` and bails
(`crates/rolldown/src/utils/load_source.rs:97-101`).

**`asset` modules currently run through the `transform` hook**, on the
placeholder string. Under this design they no longer see a placeholder there: a
plugin that today rewrites `module.exports = "__ROLLDOWN_ASSET__#0"` in
`transform` will see the asset's bytes instead, or — until `transform` is
widened to carry bytes — will not be called at all
(`crates/rolldown/src/module_loader/module_task.rs:301`). Either way this is a
breaking change, and an intentional one: intercepting another plugin's internal
placeholder was never a contract.

Taken together, the second table is the real argument for this work. There is no
rule a plugin author can learn. `this.load` gives you source for four types, the
empty string for two (one of them a bug), an encoded string for three, a
generated program for one, a synthetic CJS placeholder for one, and a hard error
for two. Nothing about a module type predicts which. The invariant replaces the
list with a rule.

### A plugin cannot produce an `asset` or `copy` module

The conversion sites are not merely misplaced — because two of them sit in
`load` and `resolve_id`, the module types they own are unreachable from any
other plugin.

`load` is a `first` hook and `AssetModulePlugin` orders itself `Post`
(`rolldown_plugin_asset_module/src/lib.rs:44-47`), so a user plugin that returns
`{ code, moduleType: 'asset' }` wins the hook and the asset plugin never runs.
`load_source` then takes its `(Some(source), Some(module_type))` branch
(`load_source.rs:53`) and hands `ModuleType::Asset` straight to
`pre_process_source`, which hard-errors: _"Encountered a module with type
`asset` during AST parsing"_ (`parse_to_ecma_ast.rs:165-171`).

The `transform` hook can also rewrite the module type — `plugin_driver.transform`
takes `module_type: &mut ModuleType` and writes through it
(`crates/rolldown_plugin/src/plugin_driver/build_hooks.rs:259,337-338`) — and a
plugin that sets `'asset'` or `'copy'` there reaches the same two error arms.

So both types are reachable only via the config-driven extension match that the
asset plugin performs on itself, or via `asserted_module_type`. A plugin can
name them but cannot make one. Under this design they become ordinary module
types: `load`/`transform` may select any type, and the conversion stage — which
runs strictly after both — dispatches on whatever they settled on.

### Whether `transform` runs is decided by content representation

`module_task.rs:284-302` runs `transform` for `StrOrBytes::Str` and skips it for
`StrOrBytes::Bytes`. `load_source` preserves a plugin-provided string when the
hook also provides an explicit `moduleType`. When the same module type is
inferred as `base64`, `binary`, or `dataurl`, it converts the string to bytes
instead. Consequently, `transform` may run or be skipped even when the resulting
module type is identical.

Today this choice is indirect: plugins cannot return bytes explicitly, but can
affect the representation by providing or omitting `moduleType`. Once `load`
can return bytes (below), the same dispatch becomes a rule the plugin controls
deliberately —

> **`transform` runs if and only if the module's content is text.**

— and the module type stops entering into it. `transform` stays text-only; see
[Future extension: binary `transform`](#future-extension-binary-transform).

## Binary content through `load`

`HookLoadOutput.code` is an `ArcStr`
(`crates/rolldown_plugin/src/types/hook_load_output.rs:6-11`), and the napi
`BindingHookLoadOutput.code` is a `String`
(`crates/rolldown_binding/src/options/plugin/types/binding_hook_load_output.rs:11-17`).
A `load` hook can only return text. This is
[#5662](https://github.com/rolldown/rolldown/issues/5662): _"Allow load hook to
return code as Buffer"_, which observes that a plugin returning binary data has
it round-tripped through UTF-8 and corrupted, and proposes widening
`SourceDescription` to `code: string | Uint8Array` to mirror `EmittedAsset`.

That issue is a **prerequisite for the invariant, not an adjacent nice-to-have.**
The invariant says `ModuleInfo.content` is what the module held after `transform`.
If `load` and `transform` cannot carry bytes, then the only modules whose content
can be binary are the ones the core read from disk itself, and the invariant has
a hole in it precisely where plugins are involved. Concretely, today:

- `{ code: "…", moduleType: 'binary' }` from a plugin reaches
  `pre_process_source`'s `Binary` arm, which calls `source.as_bytes()` on a
  `String` (`parse_to_ecma_ast.rs:183`). The bytes that get base64-encoded are
  the UTF-8 encoding of whatever survived the round trip, not the plugin's bytes.
  Same for `base64` and `dataurl`.
- There is no way for a plugin to hand rolldown an image, a wasm binary, or a
  font at all.

So `load` grows a byte arm:

- `HookLoadOutput.code: ModuleContent` (`Text(ArcStr) | Binary(Bytes)`), i.e.
  `code: string | Uint8Array` in `SourceDescription`.

`transform` does **not**, for now. It stays text-only, and byte-sourced modules
continue to skip it, exactly as `base64`/`dataurl`/`binary` do today. That keeps
this design's plugin-visible API surface to `load` and `ModuleInfo`, and it
leaves the rule stated above intact and easy to explain. The byte-capable
`transform` is scoped as a follow-up.

This preserves `transform` behavior for most module types. `base64`, `dataurl`
and `binary` already skip `transform`; `copy` never reached it, being external
at `resolve_id`; and `json`, `text` and the JS family stay text-sourced. There
are two exceptions:

- **`empty`** is text-sourced today, so `transform` runs before conversion
  discards the result. Under this design it is binary content: `this.load`
  exposes the file's bytes without decoding them, `transform` does not run, and
  conversion discards the bytes. Decoding content that will never be inspected
  would be both unnecessary and lossy for non-UTF-8 files.
- **`asset`** is represented today by the placeholder string returned from
  `AssetModulePlugin::load`, so `load_source` yields `StrOrBytes::Str` and
  `transform` runs. Once `load` reads the asset as bytes, it stops running. A
  `transform` hook filtering on `.png` is called today and will not be tomorrow.

The asset behavior change is accepted on two grounds. It is unavoidable under
the invariant — the whole point is that an asset's content is its bytes, and
`transform` cannot see bytes. And nothing is expected to depend on it:
`__ROLLDOWN_ASSET__#` is a private detail of `AssetModulePlugin`, a `transform` hook receiving
`module.exports = "__ROLLDOWN_ASSET__#<ref>"` in place of a PNG was never a
contract anyone could reasonably build on, and no plugin in this repo or in Vite
reads it. The only places that mention the prefix outside the plugin are its own
doc, one snapshot, and the dev-server tests below.

The prefix does leak in one direction, and this design does not change it: HMR
patches and lazy-compilation chunks ship the raw `__ROLLDOWN_ASSET__#<ref>`
because `render_chunk` — where the placeholder is resolved — does not run on
them (`packages/test-dev-server/tests/playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts:109,143`,
`.../lazy-compilation/__tests__/emitted-asset.spec.ts:12-14`). Conversion moving
to a proper stage neither causes nor fixes that; it remains a `render_chunk`
coverage gap.

## Design principles

**1. One conversion, one stage.** Everything that turns `ModuleType` → JS
happens inside `parse_to_ecma_ast`, entered after the `transform` hook. The
stage has a source→source front half (`pre_process_source`) and an AST→AST back
half (TypeScript/JSX lowering, in `PreProcessEcmaAst::build`). Which half a
given module type uses is an implementation detail; that it happens _in this
stage and nowhere earlier_ is the contract.

**2. `this.load` observes the input to conversion, not its output.** This is
the invariant. It is what makes `this.load` predictable: the content you get
back is the content the module type names.

**3. `ecma_view.source` stays post-conversion.** It cannot carry pre-conversion
content, because every AST span indexes into it.
Pre-conversion content is therefore a _second_ slot on the module, not a
redefinition of the existing one. Principle 2 and principle 3 are in tension by
construction, and this is how the tension is resolved.

**4. Conversion is a hook, but an internal one.** Asset conversion needs
`emitFile`; `dataurl` needs the module path for MIME guessing; `binary` needs
`options.platform`. Rather than thread all of that into a core `match`, module
conversion becomes a Rust-only plugin hook, following the precedent of
`transform_ast` (`crates/rolldown_plugin/src/plugin.rs:284-292`), which is
registered in `HookUsage` but has no JavaScript counterpart. `AssetModulePlugin`
and `CopyModulePlugin` keep their identity as plugins — see
[plugin-asset-module](../plugin-asset-module/implementation.md) for why that
matters — and simply move their work from `load`/`resolve_id` to the hook that
runs at the right time.

**5. Content is shared, never copied.**

## The conversion hook

Sketch, not a signature to implement verbatim:

```rust
pub struct HookModuleConversionArgs<'a> {
  pub id: &'a str,
  pub module_type: &'a ModuleType,
  pub content: &'a ModuleContent,
}

pub struct HookModuleConversionOutput {
  /// JavaScript source text. Parsed by the core immediately after.
  pub code: ArcStr,
  /// `code` is a bare expression rather than a program; the linker
  /// decides `export default` vs `module.exports`.
  pub has_lazy_export: bool,
}
```

`first` semantics, like `load`. The core registers converters for the builtin
module types — this is `pre_process_source` unchanged, just relocated behind the
hook — and the asset and copy plugins register theirs.

The resulting pipeline:

```
resolveId → load → transform (text sources only) → ┌ module conversion ┐ → moduleParsed
                                                   │  ├ source→source  │
                                                   │  └ parse + TS/JSX │
                                                   └───────────────────┘
                       ^                            ^
                       └ this.load observes here    └ conversion consumes it
```

### `has_lazy_export` falls out for free

`internal-docs/plugin-asset-module/implementation.md` documents that asset
modules were forced onto `module.exports = "..."` (CJS) because a plugin's
`load` hook must commit to an export style before the linker knows how the
module is consumed, and lists `LoadOutput::LazyDefaultExportExpr` as "Option 3,
future consideration."

Module conversion _is_ that future consideration. A converter that returns
`"__ROLLDOWN_ASSET__#ref-0"` — a bare string-literal expression — with
`has_lazy_export: true` lets `generate_lazy_export` pick `export default` for
ESM consumers and `module.exports` for `require()` consumers, exactly as the
old built-in did. This removes both the `__toESM` interop overhead on ESM
imports and the `side_effects: false` workaround that currently keeps a bare
`module.exports` assignment from leaking into ESM output.

## `ModuleInfo` surface

```ts
interface ModuleInfo {
  /** The module's content, after `transform` and before module conversion. */
  content: string | Uint8Array | null;
  /**
   * The module type, needed to interpret `content`.
   * `null` until the module has finished loading.
   */
  moduleType: ModuleType | null;
  /**
   * @deprecated Use `content`. A string-only view: `null` when the module's
   * content is binary.
   */
  code: string | null;
  // …unchanged
}
```

`content` is the blessed API. `code` is retained as a string-only view for
Rollup compatibility, and becomes `null` for `base64`/`dataurl`/`binary`/
`empty`/`asset`/`copy`. This gives `code === null` a new meaning: in addition to
an external or not-yet-loaded module, it may now identify a loaded module whose
content is binary. Plugins using that check to detect loading state should
inspect `content` and `moduleType` instead. This is not considered a breaking
change because binary `content` only occurs for non-JavaScript module types,
which are experimental. For consumers of those types, the behavior change is
also loud rather than silent: a plugin doing `info.code.includes(…)` gets a
null-dereference instead of quietly matching against the encoded string.

`moduleType` is new on `ModuleInfo` and is load-bearing: without it, `content`
is uninterpretable, and the future conversion API below cannot be called.

Three layers to change, following the existing `code` getter as a template:

- `crates/rolldown_common/src/types/module_info.rs:8-18` — the struct. Five
  construction sites: `normal_module.rs:83` (`to_module_info`), the early
  placeholder registration at `module_task.rs:96`, `module_loader.rs:869`,
  `scan_stage_cache.rs:296`, `external_module_task.rs:57`.
- `crates/rolldown_binding/src/types/binding_module_info.rs:64-67` — add a
  `content` getter alongside `code`. Note `code` returns `Option<&str>` borrowed
  out of the retained `Arc<ModuleInfo>` and is free; a `&[u8]` → `Uint8Array`
  conversion copies unless constructed with external data.
- `packages/rolldown/src/types/module-info.ts:14` and
  `packages/rolldown/src/utils/transform-module-info.ts:11-13`.

One trap: `PluginContextImpl.load` dedupes with
`if (moduleInfo && moduleInfo.code !== null) return moduleInfo`
(`packages/rolldown/src/plugin/plugin-context.ts:302`). Once `code` is `null`
for binary types, that guard re-loads assets on every call. It must test
`content`.

## `this.load` takes no `moduleType`

`ModuleInfo.moduleType` is the whole of the mechanism: it is what makes `content`
interpretable, and a caller who needs a particular type reads it off the result
and errors. `this.load` gains no `moduleType` parameter — as an override it would
be unsound (a module's identity is its id; `try_spawn_new_task` returns early on
`VisitState::Seen`, `module_loader.rs:320-326`, so the second caller's override
would be silently dropped), and as an assertion it would only do what the caller
can do at the call site.

`this.load` builds a `ResolvedId` with `..Default::default()`
(`native_plugin_context.rs:64-71`), so it does not inherit a type assertion from
some other import edge. This is intentional. A caller that needs a non-default
module type must arrange for `moduleTypes` or a load hook to select it. If an
unconfigured `.png` is treated as JavaScript and subsequently fails to parse,
that is a configuration error rather than something `this.load` should repair
from unrelated graph context.

## `new URL()` is not a module-type assertion

`new URL('./file', import.meta.url)` describes how the reference is consumed:
the referenced file must be emitted and the first argument rewritten to its
output URL. It does not describe how that file should be parsed if it is also
imported as a module. Module type and URL emission are independent operations.

The current scanner couples them by setting
`asserted_module_type = Some(ModuleType::Asset)` on every `ImportKind::NewUrl`
record (`new_url.rs:69`). That makes `Asset` do double duty as both a module type
and the trigger for URL emission, and makes the module's type depend on whether
the `new URL()` edge or another loader reached its id first. This coupling is
not part of the target design.

Instead, `ImportKind::NewUrl` itself should drive the three required steps:

1. resolve the referenced local file;
2. emit that file as an output asset; and
3. rewrite the URL expression to the emitted file's output URL.

This path applies regardless of the file's configured `ModuleType` and does not
need to create an `Asset` module merely to trigger emission. The same file may
therefore be imported normally according to `moduleTypes` and independently be
referenced by URL without either operation changing the other's module type.
Actual import attributes such as `with { type: … }` remain type assertions on
their import edges.

## Memory

Let a module's file be **N** bytes. "Retained" means alive for the whole build,
in `SharedModuleInfoDashMap` or the `ModuleTable`.

| type                  | today                           | proposed                     | delta |
| --------------------- | ------------------------------- | ---------------------------- | ----- |
| `js`/`jsx`/`ts`/`tsx` | source, N                       | same `ArcStr`, refcount bump | **0** |
| `json`                | `""` (the bug)                  | JSON text, N                 | +N    |
| `text`                | escaped text, ~N                | escaped ~N + raw N           | +N    |
| `base64`              | base64 literal, ~1.37N          | ~1.37N + raw N               | +N    |
| `dataurl`             | dataurl literal, ~1.37N         | ~1.37N + raw N               | +N    |
| `binary`              | generated source, ~1.37N        | ~1.37N + raw N               | +N    |
| `asset`               | placeholder + `FileEmitter`'s N | placeholder + N, **shared**  | **0** |

The types people point at large files — `asset`, `copy` — cost nothing, because
`EmittedAsset.source` already holds exactly those bytes for the whole build
(`crates/rolldown_common/src/file_emitter.rs:20-25`; `mem::take`n out only at
generate, `:336`). Making them free requires **changing
`StrOrBytes::Bytes(Vec<u8>)` to a shared buffer** (`Arc<[u8]>` or
`bytes::Bytes`, `crates/rolldown_common/src/types/str_or_bytes.rs:3-7`) so the
emitter and the module info point at one allocation. Without that refactor
`asset` becomes the _worst_ case at +N, not the best.

The +N lands on the types applied to small files: inline an icon, embed a
shim, slurp a config. That asymmetry is why this design takes the cost rather
than building a lazy re-read path — laziness would have to handle
plugin-provided content (not re-readable from disk), watch invalidation, and
`is_read_from_disk`, and it would buy nothing on the modules that are actually
big.

**Byte-sourced module types do not populate `sourcesContent`.** There is no
useful source text to show for a PNG, and suppressing it keeps output maps from
carrying a base64 copy of every inlined binary. This also removes the only
argument against shrinking `ecma_view.source` for `base64`/`dataurl`/`text`,
whose converted AST is a single string literal (`parse_expr_as_program`,
`parse_to_ecma_ast.rs:100-102`) that no diagnostic ever points into. Doing so
would put those types _below_ today's retention. It is a separable follow-up,
and `binary` cannot join it — its converted source is a real program with an
`import` record whose span must resolve.

## `copy` — the hard part

`copy` is the one type where the invariant cannot be satisfied without changing
how the module reaches the graph, because today it does not reach it at all.

Today: `resolve_id` returns `{ id: "<prefix><ref>", external: true }`, the chunk
emits `import url from "<prefix><ref>"`, and `render_chunk` rewrites the
specifier to `./f-HASH.wasm`. The import **survives into the output** and the
host resolves it at runtime. From
`crates/rolldown/tests/rolldown/function/module_types/copy/basic`:

```js
// main.js
import data from './hello.txt';
const data2 = require('./hello.txt');
```

```js
// dist/main.js
import data from './assets/hello-CUbDYdKJ.txt';
const data2 = /* …require shim… */ './assets/hello-CUbDYdKJ.txt';
```

That is the point of `copy`, and it is what distinguishes it from `asset`:
`asset` inlines a path _string_, `copy` keeps a real import of a file that was
copied verbatim.

For `this.load` to return its bytes, `copy` must enter the graph. The copy
module becomes a real module of type `Copy`; `load` reads its bytes; conversion
emits the file and returns a **proxy** backed by an external placeholder.

The proxy must preserve the consuming edge's module semantics. An ESM import
uses an ESM re-export proxy:

```js
export * from '\0copy:ref-0';
export { default } from '\0copy:ref-0';
```

A CommonJS `require()` uses a separate CommonJS proxy:

```js
module.exports = require('\0copy:ref-0');
```

The two proxies have distinct module identities because a module is deduplicated
by id and one module cannot have both ESM and CommonJS source. They are backed
by the same emitted file and reference id, so mixed `import` and `require()`
consumption must still emit the copied file only once. The external edge
survives in its original form, `render_chunk` rewrites it as before, and
`this.load` sees the bytes. The cost is an extra graph node and proxy chain for
each consumption channel a copied file uses.

The incoming edge's `ImportKind` must select the proxy identity before normal
module-id deduplication: `Require` selects the CommonJS proxy, while ESM import
kinds select the ESM proxy. Selecting after deduplication would make the first
consumer determine the representation for every later consumer and recreate
the mixed-consumption bug. The physical copy id, raw bytes, and emitted
reference remain shared backing state rather than part of either proxy's
identity.

The CommonJS proxy remains lazy. A conditional `require('./f.wasm')` enters the
proxy's CommonJS wrapper only when the original branch runs, and only then
executes the proxy's external `require`. It must not be converted into a
top-level static import or return a `__toCommonJS`-wrapped namespace. This
preserves both the current load timing and the exact value returned by the
external `require`.

### Why the proxy, and not an asset-like conversion

The obvious cheaper alternative — drop the externality and convert `copy` to a
path-string expression exactly like `asset` — is not viable, because **it does
not preserve import order.**

An external import is an _evaluated_ edge. Its position in the emitted import
list is where the host loads and runs that file, relative to every other import
in the chunk. Collapsing `copy` into a string-valued module deletes the edge:
the specifier stops being imported, so whatever the host was doing at that point
in the sequence — fetching a stylesheet, instantiating a wasm module, running a
worker's side effects — either stops happening or happens at a different time.
Nothing in the output would look wrong; the ordering just silently changes.

The proxy preserves the edge. The copy module sits in the graph where the
original import sat, and carries the external import record forward, so the
external's position in exec order is derived from the proxy's position — which
is the original importer's position. Import order is preserved by construction
rather than by coincidence.

The secondary argument against asset-like conversion still holds, and is worth
recording: `import x from './f.wasm'` would begin yielding a string rather than
performing an import, a silent runtime behavior change for every existing `copy`
user. But import order is the decisive one, because it is not fixable by
adjusting what the module exports.

### What the proxy must get right

Three properties the current external-at-`resolve_id` form gets for free, and
which the proxy has to re-establish deliberately:

- **Side-effect-only imports must survive.** `import './f.css'` has no bindings
  to re-export. The proxy module must not be tree-shaken away, or the external
  import — and with it the file's position in load order — disappears. The proxy
  needs `moduleSideEffects: true`, which is the opposite of the
  `side_effects: false` that `asset` conversion wants.
- **`require()` semantics must be preserved.** The snapshot above shows
  `require('./hello.txt')` surviving as a runtime `require` of the rewritten
  path. A `require()` edge must therefore use the CommonJS proxy
  `module.exports = require('<external-placeholder>')`; it must not pass through
  the ESM re-export proxy and rely on interop lowering. This keeps the external
  operation as `require`, preserves conditional load timing, and returns the
  external module's CommonJS value exactly.
- **`export { default }` is not enough on its own.** A copy module has no
  statically known exports; `export *` from an external is what carries an
  arbitrary shape through. Both lines are needed, and `export *` from an
  external is a construct worth confirming the linker handles as intended.

`copy` remains the natural phase-2 boundary: the invariant can land for
`json`/`text`/`base64`/`dataurl`/`binary`/`empty`/`asset` without touching it.

## What this unblocks: CSS, and Vite

The invariant is worth more to rolldown's consumers than to rolldown itself, and
CSS is the clearest case.

A downstream tool — Vite today, rolldown itself later — that wants to implement
CSS wants to call `this.load` on an asset and work with what comes back. Today
what comes back is JavaScript: `module.exports = "__ROLLDOWN_ASSET__#ref-0"`.
To get from there to a CSS `url()` reference you must parse the JS, recognize a
placeholder that is a private detail of `AssetModulePlugin`, and hope the
representation does not change. Conversion has already happened and it converted
to the wrong language.

The invariant removes the problem by not creating it. `this.load` on an asset
returns the bytes; the caller decides what target language to convert them into.
Rolldown's own asset conversion targets JavaScript because rolldown emits
JavaScript modules. A CSS pipeline targets CSS. The two are peers — the same
module type, converted for different consumers — and neither has to reverse the
other's output.

This is also the shape a native CSS module type would take when it returns
([#4271](https://github.com/rolldown/rolldown/issues/4271)). `css` is the one
row of the target-behavior table that is currently unreachable, and it is not a
coincidence: CSS bundling was removed, so `module_task.rs:117-124` errors before
the conversion stage is ever entered. But the row describes what a `css` module
type _is_ — raw CSS in, some conversion out — and the stage this design defines
is where that conversion would live. Getting `this.load` to return
`.foo { color: red }` is a precondition for either rolldown or Vite implementing
it.

One tension to note: principle 4 keeps the conversion hook internal to Rust, so
Vite cannot _register_ a CSS converter. It composes instead — `this.load` for
the content, its own `transform` for the conversion. Whether that is sufficient,
or whether the hook should eventually be exposed, is an open question below.

## Future extension: binary `transform`

`transform` is text-only, so a plugin cannot transform a module whose content is
binary — it is simply not called. The rule is coherent and matches today's
behavior for `base64`/`dataurl`/`binary`, but it is a real gap: there is no
supported way to, say, optimize a PNG or rewrite a wasm binary from a plugin.

Widening it means `HookTransformArgs.code` and the transform output become
`ModuleContent` rather than text, and every existing `transform` hook has to be
prepared for `code` to be a `Uint8Array`. That is a much larger plugin-visible
change than widening `load`, which is why it is deferred. Three shapes worth
weighing when it is picked up:

- Type `code` as `string | Uint8Array` and let every plugin narrow. Simplest,
  loudest, and breaks every plugin that does `code.replace(…)` without checking.
- Pass bytes only to plugins that opt in via a hook-meta flag, and skip the rest
  for byte-sourced modules. Backwards-compatible; adds a second dispatch axis to
  a hook that already has `order` and `filter`.
- Keep `transform` text-only permanently and give binary modules a separate
  hook. Cleanest typing; one more hook to explain.

Whichever is chosen, the rule _"`transform` runs iff the content is text"_ is
what would be relaxed, so it should be stated in the public docs now rather than
left implicit — otherwise the relaxation looks like a bug fix rather than a
feature.

## Future extension: exposing the conversion

Once conversion is a single step, it can be exposed so that a plugin which
handled a file itself can still get rolldown's conversion for it:

```ts
convert(content: string | Uint8Array, moduleType: ModuleType): string
```

so that `convert(bytes, "asset")` yields `import.meta.ROLLUP_FILE_URL_<refId>`.
Four things need settling before this is designed:

1. **It is not a pure function.** `asset` and `copy` must emit a file to have a
   `refId` at all. It has to hang off `PluginContext`, not
   `rolldown/utils`.
2. **It needs more than `(content, moduleType)`.** `dataurl` guesses MIME from
   the module path _and_ content (`guess_mime`, `parse_to_ecma_ast.rs:178`);
   `binary` picks `__toBinary` vs `__toBinaryNode` from `options.platform`
   (`:184-187`) and injects an import from `rolldown:runtime`; `json` wants the
   id for parse diagnostics. At minimum the signature needs an `id`.
3. **`ROLLUP_FILE_URL_<refId>` is not what `asset` currently produces.** It
   expands to `new URL('./rel/path', import.meta.url).href`
   (`crates/rolldown/src/module_finalizers/mod.rs:1068-1100`) — a full URL.
   Today's asset module exports a bare relative path string via the
   `__ROLLDOWN_ASSET__#` placeholder. These are different runtime values.
   Whether `asset` should migrate to `ROLLUP_FILE_URL` is a separate decision
   with its own compatibility story.
4. **`Custom(_)` module types have no conversion.** They error today
   (`parse_to_ecma_ast.rs:208-212`). `convert` should presumably reject them,
   but an internal-only conversion hook means a JavaScript plugin still cannot
   _register_ one.

## Unresolved Questions

- **The copy proxies' three properties** — the copy design is settled
  (edge-kind-specific proxy modules, for import order and module semantics), but
  the properties listed under "What the proxy must get right" are asserted, not
  verified: that a side-effect-only ESM proxy resists tree-shaking, that the
  CommonJS proxy remains lazy for a conditional `require`, returns the external
  value directly, and shares the ESM proxy's emitted file, and that `export *`
  from an external behaves as intended in the linker. Mixed ESM/`require()`
  consumption needs to exercise both proxies in one test. Each property needs a
  test before the copy phase is committed to.
- **`css` is unreachable, and how it gets reached.** CSS modules error before the
  conversion stage is entered (`module_task.rs:117-124`), so the `css` row of the
  target-behavior table is aspirational. Making `this.load` return raw CSS needs
  `load_source` to read it as text and the error to move later — a small change,
  but it means a `css` module type exists in the graph again with no conversion
  behind it. Is that acceptable as an interim state, or does `css` wait for
  [#4271](https://github.com/rolldown/rolldown/issues/4271)?
- **Conflicting import-attribute assertions still use first-seen type.** Two
  imports of one file with conflicting `with { type: … }` attributes hit the
  same `VisitState::Seen` early return, so the first assertion silently wins.
  This is an id-is-identity problem between actual module imports; `this.load`
  should not inherit an assertion, and `new URL()` no longer participates once
  URL emission is separated from module type.
- **Internal-only conversion hook, `Custom` types, and Vite.** Principle 4 keeps
  the hook out of JavaScript. That leaves no supported way for a JS plugin to
  define a conversion for `moduleTypes: { '.svg': 'my-type' }` — the natural
  reason to want one — and it means Vite implements CSS by composing `this.load`
  with its own `transform` rather than by registering a converter. Composition is
  probably sufficient; the question is whether it stays sufficient once a second
  consumer wants a non-JS target.

## Downstream doc changes

- `docs/apis/plugin-api.md:130,141-142,151` — rename the `internalTransform`
  graph node to `moduleConversion`, and reword `:151` and `:248` so that
  TypeScript/JSX lowering is described as one case of module conversion rather
  than as the whole of it.
- `internal-docs/plugin-asset-module/implementation.md` — the `has_lazy_export`
  and CJS-interop section becomes historical: "Option 3" is what this design
  implements. The `module.exports = "..."` decision and the `side_effects: false`
  workaround both go away.

## Related

- [plugin-asset-module/implementation.md](../plugin-asset-module/implementation.md) — the asset plugin as it stands, and the `has_lazy_export` trade-off this design resolves
- [ast-construction/implementation.md](../ast-construction/implementation.md) — what happens after conversion hands off source text
- [cache/design.md](../cache/design.md) — `ScanStageCache` rebuilds `ModuleInfo` from `NormalModule`, which is why pre-conversion content must live on the module rather than being set once in `module_task`
