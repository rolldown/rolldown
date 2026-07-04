# Persistent Transform Cache - Design

> The data flow and file pointers live in [implementation.md](./implementation.md).

## Problem

For most projects rolldown is fast enough that persistent caching is not worth
its complexity (see the discussion in
[#802](https://github.com/rolldown/rolldown/discussions/802)). Two situations
break that assumption (tracked in
[#6995](https://github.com/rolldown/rolldown/issues/6995)):

- Slow JavaScript `transform` plugins (framework compilers, legacy
  transpilers). Every call crosses the NAPI boundary and runs single-threaded
  JS, so the transform pipeline dominates cold-build time.
- Very large module graphs (100k+ modules in monorepos), where even cheap
  per-module work adds up and CI machines rebuild the same unchanged modules
  thousands of times a day.

The existing `ScanStageCache` only helps rebuilds within one process (watch
mode / HMR). Nothing survives a process restart, and nothing can be shared
between machines.

## Goals

- Opt-in, experimental persistent cache that skips the plugin `transform`
  pipeline for unchanged modules, across processes.
- Never fail or corrupt a build: every cache error degrades to a miss, every
  write is atomic.
- On-disk layout simple enough that external tooling can implement **remote
  caching** on top (e.g. read-through from an HTTP mirror, CI writers
  populating an object store) by syncing files, without understanding the
  entry format. Cache keys must therefore be machine-portable (no absolute
  paths).
- Smallest possible surface area inside rolldown; the cache touches exactly
  one seam (around `transform_source`).

## Non-goals

- Caching parse, link or codegen results. The oxc AST is arena-allocated and
  self-referential, so it cannot be serialized without a dedicated
  representation; parsing is also cheap relative to JS transform hooks.
- Automatic invalidation on plugin configuration or implementation changes.
  Rolldown cannot observe either, so callers fold a config/lockfile hash into
  `experimental.transformCache.key` (the same contract Metro's
  `getCacheKey` and webpack's `cache.version` use).
- Replacing `ScanStageCache`. The two compose: within a watch session the
  in-memory cache avoids re-scanning entirely; the persistent cache
  accelerates the scans that do happen.

## Decisions and trade-offs

- **Cache the post-transform source, not scan output.** Deserializing symbols,
  scopes and statement info would require serializable mirrors of large
  internal types for modest wins; re-parsing with oxc is fast. Skipping the
  transform pipeline captures most of the win at a fraction of the surface.
- **Key on the post-`load` source.** `load` hooks still run on hits (they are
  the source of truth for virtual modules and cheap for disk files), which
  makes the key well-defined for every module kind.
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
- **Side effects are stored as a delta.** The pre-transform value can come
  from resolution data outside the cache key (`package.json` `sideEffects`),
  so only a transform-hook override is stored and replayed.

## Known limitations

- Transform-hook side channels are not replayed on hits: `this.emitFile`,
  `this.addWatchFile` and custom module meta written during `transform` are
  lost for cached modules. Plugins relying on those must currently not be used
  with the cache enabled.
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
