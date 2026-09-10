# `import.meta.glob` — Design & Principles

## Summary

`crates/rolldown_plugin_vite_import_glob` expands `import.meta.glob(...)` into a literal object of
imports at `transform` time. That is a snapshot of the filesystem, so in dev mode the snapshot has to
be refreshed whenever a file that matches the glob appears or disappears — otherwise a new route,
locale, or content file stays invisible until the server restarts.

This doc records why the refresh works the way it does. The machinery is in
[implementation.md](./implementation.md).

## Where the responsibility sits

Unbundled Vite gets most of this for free: chokidar watches the project root recursively, and the JS
plugin (`packages/vite/src/node/plugins/importMetaGlob.ts`) only has to answer "which modules care
about this file?" from a `hotUpdate` hook.

Under Vite's bundled dev (full bundle mode) neither half is free:

- `importGlobPlugin`'s `applyToEnvironment` replaces the whole JS plugin with the native one for the
  bundled environment, so the `hotUpdate` hook disappears with it.
- Vite's chokidar stops driving HMR — `hmr.ts` returns early on `config.experimental.bundledDev` and
  leaves file events to rolldown's own dev-engine watcher, which registers **individual files** from
  the module graph. A file that is not in the graph yet cannot produce an event at all.

So the native plugin owns both halves: it must put the directories it read into the watch set, and it
must implement `hotUpdate`. See [rolldown#10059](https://github.com/rolldown/rolldown/issues/10059).

## Principles

1. **The hook replays the build-time walk, it does not re-implement it.** `GlobMatcher` stores the
   same inputs the walk used (walk root, the `(static prefix, pattern)` split, `exhaustive`,
   `caseSensitive`) and reaches its verdict with the same `fast_glob` call and the same pruning rules.
   Two independent predicates would drift, and both directions of drift are bugs: a laxer hook
   re-runs modules whose glob output cannot have changed, a stricter one silently drops updates.

   This is also why the hook does **not** copy vite's matcher verbatim. Vite's matcher treats "only
   negative patterns" as matching everything (`affirmed.length === 0 || affirmedMatcher(file)`), while
   rolldown's walk yields nothing for that input. Following vite there would make the hook wider than
   the build.

2. **Watch the directories the walk actually visited, not the glob's root recursively.** A recursive
   watch is one syscall on macOS but one inotify watch per subdirectory on Linux, and it cannot be
   told to skip `node_modules`. The walk already enumerates exactly the directories the glob reads,
   with dotfiles and `node_modules` pruned, so reusing its output costs nothing extra and keeps the
   watch set proportional to what the glob covers.

   The gap this leaves is a directory created _below_ an already-watched one: it is not watched, so
   nothing inside it will ever produce an event. What is delivered is the new directory's own
   creation, through its parent's watch — and `GlobMatcher::may_gain_matches_below` claims the module
   for it even though a directory matches no pattern. The round that follows re-walks, picks up
   whatever already landed inside, and registers the directory for what lands later. One rebuild of
   latency, and it is a rebuild the round was going to do anyway.

   That predicate is deliberately gated on the path actually being a directory (one `is_dir` call per
   round, and only when the exact predicates all miss). Without the gate, every stray file under a
   `**` glob's tree would invalidate the module — which is precisely the imprecision the pruned watch
   set exists to avoid. Deleted directories need no equivalent: their children are removed first, and
   those events are exact matches.

3. **A missing glob root is watched through its nearest existing ancestor.** `walkdir` yields nothing
   for a directory that does not exist, which would leave `import.meta.glob('./pages/*.vue')`
   permanently blind if `pages/` is created later — a normal scaffolding flow. Watching the closest
   existing ancestor makes `mkdir pages` observable, and `GlobMatcher::touches_base` is what turns
   that event into an invalidation. The ancestor is watched non-recursively, so the cost is bounded
   even when several path segments are missing.

4. **Only dev mode pays for any of this.** `hotUpdate` is a dev-only hook, so outside dev mode the
   predicate table and the directory registrations would be pure overhead — and the directories would
   also show up in `this.getWatchFiles()` for ordinary builds. Both are gated on
   `options().is_dev_mode_enabled()`.

5. **The predicate table is maintained per module, never globally reset.** `buildStart` is the obvious
   place for a reset (vite clears its map there) but it is wrong here: rolldown calls `buildStart` for
   _every_ scan, including the partial scans that re-transform only the changed modules. A reset would
   drop the predicates of every module the scan left alone. Since `transform` runs on every re-fetch,
   upserting and removing per module is both accurate and self-healing — it also covers a user
   deleting the `import.meta.glob` call.

6. **The hook adds to the affected set, it does not replace it.** Same as vite's
   `[...oldModules, ...modules]`. A file can be both a glob match and a module in its own right; the
   glob owner joining the update must not push the file's own update out of it.

## Unresolved questions

- **`rolldown --watch` is not covered.** `hotUpdate` only runs in the dev engine, so a plain watch
  build still misses new glob matches. Closing that would mean teaching `rolldown_watcher` to consult
  the glob predicates directly, or giving `hotUpdate` a meaning outside dev mode.
- **`caseSensitive: false` is ASCII-only**, in the hook exactly as in the walk: `fast_glob` has no
  nocase flag, so both sides are lowercased. Character-class ranges (`[A-Z]`) and non-ASCII case
  folding can diverge from picomatch's `nocase`.
- **The watch set only grows.** The dev coordinator never unwatches, so a directory that stops being
  covered by any glob stays watched for the life of the server. This is the pre-existing watch-mode
  behaviour (see `../watch-mode/implementation.md`), not something this feature adds.

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../dev-engine/implementation.md` — where `hotUpdate` runs in the HMR stage
- `../watch-mode/implementation.md` — how watch files reach the fs watcher
- [rolldown#10059](https://github.com/rolldown/rolldown/issues/10059) — the feature request
- [rolldown#10019](https://github.com/rolldown/rolldown/issues/10019) — full bundle mode phase 2
