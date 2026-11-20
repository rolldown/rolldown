# Runtime Helpers

## `__commonJS` and `__commonJSMin`: release `cb` after initialization

After the first call in either helper, `mod` is set and `cb` is never accessed again. Without an explicit `cb = null`, the factory is permanently retained in the closure — a memory leak in long-lived processes (e.g. SSR servers loading bundles via `vm.createContext`).

Reference: https://github.com/rolldown/rolldown/issues/9063

## `__exportAll`: namespace objects with the native shape

`__exportAll(all, flags)` creates the namespace object of a module. `all` maps each export name to a getter, so code that reads an export sees the live binding. The object has the shape of a native module namespace:

- It has a null prototype. So `'constructor' in ns` is `false`, unless the module exports `constructor`.
- It has `Symbol.toStringTag` with the value `'Module'`, unless the flag `1` is set (`generatedCode.symbols: false`).
- It accepts no new properties (`Object.preventExtensions`). In the specification, [[IsExtensible]] of a module namespace object returns `false`.

The flag `2` keeps the object open: the object accepts new properties. A later `__reExport` adds properties to the namespace object of a module with dynamic exports or with an external `export *`. On an object that is not extensible, `Object.defineProperty` throws a `TypeError` for a new property. `namespace_object_is_open` in `module_finalizers/mod.rs` decides which namespace objects stay open. The HMR finalizer passes the flag `2` for a module with `export * from`. An open object also has a null prototype.

A writer is a place in the module finalizer that emits a `__reExport` call into a namespace object. Each writer checks `namespace_object_is_open` with a debug assertion. `namespace_object_is_open` has one condition for each kind of writer:

- The writer for an ESM importee with dynamic exports and the writer for a CommonJS importee read the current flags. They emit `__reExport` only for an `export *` of a module with dynamic exports. Each pass that gives a module dynamic exports also gives dynamic exports to each module that has an `export *` of that module. These passes are `determine_has_dynamic_exports` in `link_stage/wrapping.rs` and `find_entry_level_external_module` in `generate_stage/code_splitting.rs`.
- The writer for an external `export *` reads `star_exports_from_external_modules`, not `has_dynamic_exports`. `find_entry_level_external_module` ignores the records that it marks with `EntryLevelExternal`. So a module can lose its dynamic exports while this writer still emits `__reExport(ns, <external>)`. The fixture `misc/namespace_object_shape_open_flattened_external_star` shows this case.
- The overlay writer of strict execution order reads `reexports_dynamic_exports`, a copy that `apply_order_wraps` makes of the `has_dynamic_exports` flag of the importee. `namespace_object_is_open` also returns `true` when an overlay of the module has `reexports_dynamic_exports`. So the object stays open when the overlay writer emits `__reExport`. Today two facts make this condition not necessary:
  - When tree shaking keeps a module, it keeps each `export *` of a module with dynamic exports. `reference_needed_symbols` gives such a statement a side effect, and `is_gated_side_effect_stmt` is always `false` for a statement with an import record. The overlay writer runs only for an `export *` that tree shaking removes, so it never emits `__reExport`.
  - `finalize_chunk_plan` runs `find_entry_level_external_module` again after `apply_order_wraps`, but each later call gives the same flags as the copy. No pass removes an `EntryLevelExternal` mark or an `Unknown` namespace reason, and no pass after the first call adds an entry module. So `has_dynamic_exports` also covers the overlay writer.

No build runs the overlay writer. So the unit tests in `module_finalizers/mod.rs` call `is_namespace_object_open` with an `OrderWrapState` that they make. In this state, the flags of the module say that the object is closed, and an overlay of the module has `reexports_dynamic_exports`.

An empty namespace with `generatedCode.symbols: false` does not call `__exportAll`, because the link stage does not always include the helper in this case. Its value is `Object.freeze({ __proto__: null })`. When the object is open, its value is `Object.setPrototypeOf({}, null)`.

`__exportAll` creates the object with `Object.setPrototypeOf({}, null)`, not with `{ __proto__: null }` or `__create(null)`. V8 keeps the properties of an object from `{ __proto__: null }` or `Object.create(null)` in a hash table (dictionary mode). So code that reads an export from such a namespace object runs slowly. V8 does not put an object from `Object.setPrototypeOf({}, null)` in dictionary mode, also after `__reExport` adds getters. The frozen empty object still uses `{ __proto__: null }`, because it has no properties.

In Node 26, a loop of 5e7 iterations that reads two exports took about 700 ms with `{ __proto__: null }`. The same loop took about 50 ms with `Object.setPrototypeOf({}, null)` or with an ordinary object. JavaScriptCore gives the same speed for all three forms.

These differences from a native namespace object stay:

- Each export is a getter, not a data property with `writable: true`. So `Object.getOwnPropertyDescriptor(ns, name)` returns a getter. `Object.isFrozen(ns)` returns `true`, but it returns `false` for a native namespace object with exports.
- An open object accepts new properties.

These fixtures check the shape: `misc/namespace_object_shape`, `misc/namespace_object_shape_empty`, `misc/namespace_object_shape_open` and `misc/namespace_object_shape_open_flattened_external_star`. The Rust test `test262_module_code` runs the test262 tests of `language/module-code`, and the tests in `namespace/internals` also check the shape. `crates/rolldown/tests/test262_failures.json` lists each namespace test that still fails, with its reason.

## `__copyProps`: getters without `Function.prototype.bind`

`__copyProps(to, from)` defines a getter on `to` for each own property of `from`, so a read sees the live value. `__toESM`, `__reExport`, and `__toCommonJS` use it. So code reads through these getters for `import * as ns` of a CommonJS module, for `default` when the module sets `__esModule` and has its own `default`, for `export *` of a CommonJS module, and for `require()` of an ES module. A named import of a CommonJS module reads `module.exports` directly and does not use these getters.

Each getter is a closure `() => from[key]`, or the getter of `from` itself. It is not a bound function such as `((k) => from[k]).bind(null, key)`:

- V8 and SpiderMonkey do not cache a property read that calls a bound function as a getter. The load inline cache of V8 handles only a getter that is a `JSFunction` or a `FunctionTemplateInfo`, and `IsCacheableGetPropCall` in SpiderMonkey handles only a `JSFunction`. In a microbenchmark, a read through a bound getter took 75 to 90 ns in V8 and SpiderMonkey, and 4 to 5 ns in Safari 26, Safari 27, and Bun 1.4. A read through a closure took less than 1 ns. JavaScriptCore inlines a bound getter since 325006@main (WebKit bug 329028).
- When the property of `from` is a non-configurable accessor, the copy uses the same getter. A non-configurable accessor cannot change, so the copy still sees the live value. Without this rule, a read calls two getters in these cases: `__toCommonJS` copies the getters of an `__exportAll` namespace, `__reExport` copies the getters of a `__toESM` namespace, and the CommonJS output of TypeScript defines non-configurable getters for re-exports. When one read site reads many modules, `from[key]` in the closure misses the inline cache: in JavaScriptCore trunk, a read through two closures took about 44 ns, a read through two bound functions about 1 ns, and a read through one getter 0.2 ns.
- The loop is `for (let key of ...)`, so each closure has its own `key`. (esbuild uses `.bind(null, key)` only in the runtime for ES5, where `var key` is shared by all iterations.) The runtime already uses arrow functions and `let`, so this form needs nothing newer.

A reused getter runs with `this` set to `to`, not to `from`. A getter that reads another property through `this` gets the same value when `__copyProps` copied that property to `to`. It reads a different value for a name that `to` already has, such as an export that an ES module declares next to `export * from` a CommonJS module, for `default` when it is not copied, for a symbol key, and for an inherited property unless `__toESM` created `to` with the prototype of `from`. A getter that compares `this` with `from` also sees a different object.

These costs stay:

- In JavaScriptCore, many getters share the code of one closure. So the inlined getter keeps the `GetClosureVar` loads of `from` and `key`. In bundle benchmarks, a read took 0.6 to 2.2 ns more than through a bound getter in JavaScriptCore trunk. Safari 26, Safari 27, and Bun 1.4 do not inline bound getters, so the closure is faster there.
- V8 gives an object dictionary-mode properties when the object gets an accessor that has the name of an existing map transition but a different getter (`TransitionToDifferentAccessor` in `Map::TransitionToAccessorProperty`). TypeScript and Babel output defines `__esModule` first, so the namespace objects of the second and later such modules are in dictionary mode. A read from them takes about 12 to 18 ns.

The fixture `misc/copy_props_getters` checks the getters.

## `__share`, `__share_require`, `__share_export`: the inline common chunk registry

`experimentalInlineCommonChunks` (see `internal-docs/inline-common-chunks/`) prints a common chunk's modules into the files that read them, wrapped in a factory. The registry makes those copies one module instance:

- `__share(id, factory)` — records `factory` under `id` unless an id is already registered. The first registration wins; a page that loads two carriers keeps the first factory it saw and ignores the second. Registration never runs the factory.
- `__share_require(id)` — the five states. _Unregistered_: throws `Error("Shared chunk \"<id>\" was not registered before it was required.")`; along supported loading paths, the bundler places a registration before every require, earlier in the same file, in a static dependency outside the file's import cycle, or on every known prior entry loading path. Loading through paths outside the build graph can reach this state (see `inline-common-chunks/design.md`, Supported loading paths). _Registered_: creates the record `{ exports: {}, failed: false, error }`, stores it before calling the factory, and calls `factory(exports)`. _Executing_: a nested require of the same id (a cycle between records) returns the same, partially filled `exports` object. _Completed_: returns the same `exports` object every time. _Failed_: a factory that threw is never re-run; the same thrown value is rethrown, including `undefined` and `null` (`failed` is a separate flag so `throw undefined` is not mistaken for success).
- `__share_export(target, all)` — installs `all`'s entries as enumerable getters on `target`, like `__exportAll`, so readers see live bindings.

The factories only define wrappers (`init_*`, `require_*`) and the getter table; user code still runs inside the wrappers, so a failure while a shared module executes is reported by `__esmMin`/`__commonJS` as for any wrapped module. The registry is a binding of the runtime module rather than a `globalThis` property: files share it exactly when they import the same runtime chunk URL, which two builds that emit an identical runtime file into one directory do (see `internal-docs/inline-common-chunks/design.md`, Unresolved questions), and it takes part in tree shaking, naming and hashing like every other helper.

## `__toESM`: deciding interop for external modules

`require("external")` returns the raw CommonJS exports, so non-ESM formats must run it through `__toESM` whenever the bundle reads it _as an ES module_ — `import * as ns`, or `ns.default`. A named-only import reads the CommonJS object directly and must not be wrapped.

The trap is that the question cannot be answered from the static imports a chunk's own modules carry. `import d from 'external'; export { d }` links the shim's symbol to a `NamespaceAlias`, so tree-shaking follows the alias straight to the external namespace and never enqueues the shim's declaring statement. With a side-effect-free external the shim is dropped entirely, while the `<external_ns>.default` reference it produced survives elsewhere. Deriving `needs_interop` from `chunk.direct_imports_from_external_modules` then answers "no" for a chunk the finalizer still renders `.default` against — a silently wrong bundle (issue #10069).

So the **inclusion pass records it** (`note_external_interop_use`), being the one place that walks the reference _after_ linking resolved it. Three consumers OR that record into their own `named_imports`-derived answer: the cjs renderer, the iife/umd renderer, and the chunk deconflicter's mixed-mode naming — all via `chunk_recorded_external_interop`.

Two things follow from the importer being potentially dead:

- **Keeping the helper alive.** `RuntimeHelper::ToEsm` is normally requested by the import statement, so it dies with the module holding it. `include_statements` re-adds it whenever any interop use was recorded, and `patch_module_dependencies` derives the runtime _edge_ from the reference rather than the importer — including references that exist only as entry exports (`referenced_symbols_by_entry_point_chunk`), which are otherwise invisible to the statement walk and leave the chunk emitting a helper it never imported.
- **Node-mode provenance.** `__toESM(mod, 1)` applies when the importing module is ESM by definition format. Linking collapses the re-export chain onto the external's namespace symbol, so the reference reaching the inclusion pass may belong to a consumer several hops downstream; the importing module is recovered by walking the symbol link chain (`external_import_writer`).

Inclusion runs before chunking, so the record cannot name a chunk directly. It stores the **observer** instead — the module whose included code holds the surviving reference — and rendering resolves that to a chunk via `ChunkGraph::module_to_chunk`. The observer is deliberately not the importer: the importer is routinely the shim that gets dropped, whereas an observer has included code by construction and therefore lands in a chunk.

`chunk_recorded_external_interop` then answers per chunk:

- **Which chunks wrap.** Only those an observer landed in. A chunk holding just a named import of the same external reads the CommonJS object directly. Wrapping it would not break value reads — `__copyProps` installs forwarding getters — but `__toESM` returns a fresh object (`__create(__getProtoOf(mod))`) and eagerly walks `__getOwnPropertyNames`/`__getOwnPropertyDescriptor`, changing namespace identity and descriptor shape, and firing extra `ownKeys` traps on a Proxy-based export.
- **Which mode.** Modes are aggregated per observer, not bundle-wide. A `.mjs` and a `.js` module importing the same external into separate chunks each observe it in one mode; unioning across the bundle would make both chunks look mixed-mode and emit `__toESM(mod, 1)` _and_ `__toESM(mod)`, running that eager walk twice for one live binding.

An observer that ends up without a live chunk cannot be attributed, so every chunk emitting the external honours it — contributing only that observer's mode, not the bundle union. This fallback is load-bearing: narrowing it away under-approximates, which is what produced #10069.

A chunk that wraps also needs `__toESM` in its depended symbols, and the two decisions have to agree. Over-supplying costs output: a module that gains the edge without wrapping puts `__toESM` in its chunk's depended symbols, and the resulting cross-chunk import loses its binding to DCE while the side-effectful `require` of the runtime chunk survives — a bare `require("./rolldown-runtime.js")` for a helper nothing calls. Under-supplying is worse: a chunk that wraps without the edge references a helper it never imported, and finalization panics looking up the missing cross-chunk binding.

They agree because the wrap arrives through exactly **two channels**, each carrying its own edge:

- **Recorded observations.** `chunk_recorded_external_interop` wraps the chunks observers landed in, and `patch_module_dependencies` narrows the edge to those same observers. Entry-export references belong here and reach neither the statement walk nor any `named_imports`, so `note_external_interop` runs over `referenced_symbols_by_entry_point_chunk` too — dropping that call panics (`external_interop_default_reexport_only_reachable_via_entry_export`).
- **Statically written imports.** `chunk_external_interop_modes` also wraps on the chunk's own `named_imports`, with no liveness filter: a default or namespace import whose binding nothing reads still wraps, and records no observer. The same `specifier_needs_interop` predicate supplies its edge twice over — `reference_needed_symbols` registers `__toESM` against the kept import statement, and `compute_chunk_imports` re-adds the cross-chunk import for any chunk whose direct external imports need interop (`external_interop_dead_default_import_keeps_helper`).

So the invariant is pairwise, not one source answering both questions. Narrowing a channel's wrap without its edge — or the more tempting inverse, registering `ToEsm` only for imports tree-shaking kept alive — reopens the panic for the cases the other channel does not cover.

Chunks do not exist yet when `patch_module_dependencies` runs, so `is_included` stands in for "will have a chunk". An observer that is not included cannot be attributed to one, and rendering falls back to wrapping every chunk emitting the external — so every referencing module keeps the edge, matching that fallback.

## The helpers of `import defer`: only with the option

The helpers of `import defer` (`__deferNs`) are in `crates/rolldown/src/runtime/runtime-defer.js`, not in `runtime-base.js`. `RuntimeModuleTask` adds the file to the runtime source only when `experimental.importDefer` is on. Each build parses and scans the full runtime source, also the helpers that tree shaking removes later, and the `transform` hook of a plugin gets that source. With the helpers in `runtime-base.js`, a build without the option would pay for them and give a different runtime source to plugins.

The generator (`tasks/generator/src/generators/runtime_helper.rs`) reads the file last, so the other helpers keep their bits in `RuntimeHelper`. It also writes the names of the file to `RUNTIME_IMPORT_DEFER_HELPER_NAMES`. When the option is off, the module loader leaves these names out of the runtime symbol check, and no code path may resolve them: `RuntimeModuleBrief::resolve_symbol` panics on a missing name.
