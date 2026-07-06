# Persistent Build Cache - Design

> The data flow and file pointers live in [implementation.md](./implementation.md).

## Problem

For most projects rolldown is fast enough that persistent caching is not worth
its complexity (see the discussion in
[#802](https://github.com/rolldown/rolldown/discussions/802)). Two situations
break that assumption (tracked in
[#6995](https://github.com/rolldown/rolldown/issues/6995)):

- Slow JavaScript plugin hooks (framework compilers, legacy transpilers,
  custom resolvers). Every call crosses the NAPI boundary and runs
  single-threaded JS, so the `resolveId`/`load`/`transform` pipeline dominates
  cold-build time.
- Very large module graphs (100k+ modules in monorepos), where even cheap
  per-module work adds up and CI machines rebuild the same unchanged modules
  thousands of times a day.

The existing `ScanStageCache` only helps rebuilds within one process (watch
mode / HMR). Nothing survives a process restart, and nothing can be shared
between machines.

An earlier iteration ([#10137](https://github.com/rolldown/rolldown/pull/10137))
cached only the `transform` stage. Review feedback was that rolldown targets
caching for the whole bundler pipeline, not a single hook, so this design
covers everything a module task does before the native parse: the `load`
pipeline, the `transform` pipeline and dependency resolution (`resolveId`
hooks plus the native resolver).

## Goals

- Opt-in, experimental persistent cache that skips every plugin hook and the
  resolver for unchanged modules, across processes.
- Never fail or corrupt a build: every cache error degrades to a miss, every
  write is atomic.
- Warm builds must produce byte-identical output to cold builds, including
  sourcemaps and side-effect-driven tree shaking.
- On-disk layout simple enough that external tooling can implement **remote
  caching** on top (e.g. read-through from an HTTP mirror, CI writers
  populating an object store) by syncing files, without understanding the
  entry format. Cache keys and entry contents must therefore be
  machine-portable (no absolute paths).
- Smallest possible surface area inside rolldown; the cache touches exactly
  one seam (`ModuleTask::run_inner`).

## Non-goals

- Caching parse, link or codegen results. The oxc AST is arena-allocated and
  self-referential, so it cannot be serialized without a dedicated
  representation; parsing and scanning are native and fast relative to JS
  plugin hooks. Re-parsing the cached post-transform source reproduces the
  scan state, including the import records the cached resolutions align with.
- Automatic invalidation on plugin configuration or implementation changes.
  Rolldown cannot observe either, so callers fold a config/lockfile hash into
  `experimental.buildCache.key` (the same contract Metro's `getCacheKey` and
  webpack's `cache.version` use).
- Replacing `ScanStageCache`. The two compose: within a watch session the
  in-memory cache avoids re-scanning entirely; the persistent cache
  accelerates the scans that do happen.

## Decisions and trade-offs

- **Key on the module's raw on-disk content, not the post-`load` source.**
  Keying on post-`load` source (as #10137 did) forces `load` hooks to run on
  every build. Reading the file directly makes the key available before any
  hook runs, so hits skip the whole pipeline. The cost: modules that do not
  map to a real file (virtual modules, data URLs) bypass the cache entirely,
  and `load`/`transform` hooks are assumed to be deterministic functions of
  the module's id and content, the same determinism contract Metro and
  webpack's filesystem cache rely on.
- **Cache resolved dependencies positionally, not by specifier.** Scanning the
  cached code is deterministic, so the import records of a warm build are
  identical to the cold build's and a plain array lines up with them. A length
  mismatch (corrupt entry) falls back to fresh resolution.
- **Validate cached resolutions by stat-ing dep files on hits.** A deleted or
  moved dependency turns the entry into a miss, so the importer re-resolves
  and fails (or externalizes) exactly like a cold build. Newly added files
  that would shadow an old resolution are NOT detected; that class of change
  is folded into `key` (documented limitation, same as webpack's
  `unsafeCache`).
- **Serialize the full `ResolvedId`, including `package.json` data.** Side
  effects derived from `package.json` `sideEffects` globs are evaluated in the
  dependency's own task via `resolved_id.package_json`, so dropping it would
  change tree shaking between cold and warm builds. `PackageJson` is five
  small fields; paths inside entries are stored cwd-relative for portability.
- **Modules whose resolution produced warnings are never stored.** Warnings
  (e.g. "treating missing import as external") are emitted during resolution
  and would silently disappear on hits; re-running the pipeline keeps them.
- **One file per entry, content-addressed, atomic rename.** Rejected a single
  store file / database: per-entry files need no locking across concurrent
  builds, tolerate partial syncs, and are trivially mirrored to remote
  storage. The 256-way `key[0..2]` sharding keeps directories small at
  monorepo scale.
- **Format: JSON metadata header + raw code bytes.** Rejected new
  serialization dependencies (bincode/rkyv); `serde_json` is already a
  dependency, and keeping the code blob out of JSON avoids escaping the
  largest field. A format version byte plus the salt guard both decode
  compatibility and semantic compatibility.
- **Side effects are stored as a delta.** The pre-`load` value comes from
  resolution data outside the cache key, so only a load/transform-hook
  override is stored and replayed.

## Known limitations

- Hook side channels are not replayed on hits: `this.emitFile`,
  `this.addWatchFile` and custom module meta written during `load` or
  `transform` are lost for cached modules. Plugins relying on those must
  currently not be used with the cache enabled. (The module's own watch-file
  registration is preserved; it does not depend on hooks.)
- Resolution inputs that live outside module files (lockfile changes that
  move a package, `tsconfig.json` path mappings, new files shadowing an old
  resolution) are invisible to the content-based key. Callers fold them into
  `key`.
- Incompatible with `experimental.nativeMagicString` + sourcemaps: that path
  generates transform sourcemaps on a side channel outside the module task, so
  the cache silently disables itself there.
- No eviction. Entries are small and content-addressed; callers can wipe or
  age out the directory externally. Revisit if this graduates from
  experimental.

## Open questions

- Should the store become a trait (get/set) so embedders can plug remote
  backends directly instead of syncing the directory? The current layout was
  chosen so that this can be added later without changing the entry format.
- Whether Vite should derive `key` automatically from its resolved config.
- Whether entry-point resolution (`resolve_user_defined_entries`) is worth
  caching too; it is a handful of resolutions per build.
