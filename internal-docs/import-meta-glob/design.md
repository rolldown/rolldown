# `import.meta.glob` — Design & Principles

## Summary

`crates/rolldown_plugin_vite_import_glob` expands `import.meta.glob(...)` into a literal object of
imports at `transform` time. That is a snapshot of the filesystem. In dev mode the snapshot has to be
refreshed when a file that matches the glob appears or disappears. Otherwise a new route, locale or
content file stays invisible until the server restarts.

This doc records why the refresh works the way it does. The machinery is in
[implementation.md](./implementation.md).

The refresh runs through the `hotUpdate` hook, which is off by default. See
[Unresolved questions](#unresolved-questions).

## Where the responsibility sits

Unbundled Vite gets most of this for free. chokidar watches the project root, and the JS plugin
(`packages/vite/src/node/plugins/importMetaGlob.ts`) only has to answer "which modules care about
this file?" from a `hotUpdate` hook.

Under Vite's bundled dev (full bundle mode) neither half is free:

- `importGlobPlugin`'s `applyToEnvironment` replaces the whole JS plugin with the native one for the
  bundled environment, so the `hotUpdate` hook disappears with it.
- Vite's chokidar stops driving HMR. `hmr.ts` returns early on `config.experimental.bundledDev` and
  leaves file events to rolldown's own watcher, which watches the files of the module graph and what
  plugins add with `addWatchFile`. A file that is not in the graph yet is only reported if a watched
  directory contains it.

So the native plugin owns both halves: it must put the directory it reads into the watch set, and it
must implement `hotUpdate`. See [rolldown#10059](https://github.com/rolldown/rolldown/issues/10059).

## Principles

1. **The hook asks what vite's hook asks.** `GlobMatcher` is the matcher of vite's
   `vite:import-glob` plugin, with globstar in the place of picomatch. It holds one union of the
   affirmed globs and one of the negated globs, all absolute. `exhaustive` turns `dot` on,
   `caseSensitive: false` turns case folding on, and `**/node_modules/**` is excluded unless
   `exhaustive` is set. A created or deleted file concerns a module if one of its matchers matches
   the file.

   One input differs from vite. Vite treats "only negative patterns" as matching everything
   (`affirmed.length === 0 || affirmedMatcher(file)`), while rolldown's walk returns before it
   starts for that input and records no matcher.

2. **The matcher is a second predicate next to the walk, and the only state.** The walk still
   decides with `fast_glob` on a split path and prunes with `filter_entry`. The matcher decides with
   globstar on the whole path. Vite has the same pair, tinyglobby for the walk and picomatch for the
   hook. Where the two disagree, a module is updated although its output did not change, or an
   update is missed until the module is transformed again.

   The hook does not remember which files the last walk matched. So it cannot tell a new file from
   one that is created again, and it does not know what was below a deleted directory. Both are
   listed under [Unresolved questions](#unresolved-questions).

3. **Watch the walk root, once.** The watcher watches a directory with everything below it and
   reports only files: a directory that appears is reported as the files inside it. So the walk root
   covers every directory the walk reads, including the ones created later.

   The watch also covers what the walk prunes below the root, dot entries and `node_modules`. The
   matcher rejects their events. A `watch.exclude` that covers them keeps the watcher out of them,
   see `../watch-mode/implementation.md`.

4. **A missing walk root is watched through its topmost missing directory.** `walkdir` yields
   nothing for a directory that does not exist, which would leave `import.meta.glob('./pages/*.vue')`
   blind if `pages/` is created later, a normal scaffolding flow. The watcher can wait for a missing
   path, but only if its parent exists. For `./a/b/*.js` with `a/` missing, that path is `a/`.

5. **Only dev mode pays for any of this.** `hotUpdate` is a dev-only hook, so outside dev mode the
   matchers and the watched directory would be pure overhead, and the directory would show up in
   `this.getWatchFiles()` for ordinary builds. Both are gated on `options().is_dev_mode_enabled()`.

6. **The matchers are maintained per module, never reset globally.** `buildStart` is the obvious
   place for a reset (vite clears its map there), but rolldown calls `buildStart` for every scan,
   including the partial scans that transform only the changed modules again. A reset would drop the
   matchers of every module the scan left alone. `transform` runs on every fetch, so replacing and
   removing per module is accurate, and it covers a user deleting the `import.meta.glob` call. A
   module deleted from disk never reaches `transform` again, so its matchers stay in the table.

7. **The hook adds to the affected set, it does not replace it.** Same as vite's
   `[...oldModules, ...modules]`. A file can be both a glob match and a module in its own right. The
   glob owner joining the update must not push the file's own update out of it.

## Unresolved questions

- **The `hotUpdate` hook is off by default.** `dev.hotUpdate` stays off until file-to-module
  invalidation is complete ([rolldown#10714](https://github.com/rolldown/rolldown/pull/10714)), and
  Vite does not pass the option yet ([vite#22956](https://github.com/vitejs/vite/pull/22956)). Until
  then a new file does not reach the glob under Vite. A deleted file does, through the importers of
  its own module. The playground turns the hook on with `experimental.devMode.hotUpdate`.
- **`rolldown --watch` is not covered.** `hotUpdate` only runs in the dev engine, so a plain watch
  build still misses new glob matches.
- **A file saved by rename is reported as created.** An editor that writes a temporary file and
  renames it over the target makes the watcher report `Create` for a file that is already in the
  result. The matcher matches it, so the module of the glob is updated with every such save although
  its output is the same. A module the hook selects always ships, because it is exempt from the
  unchanged-output suppression. Vite does not see this, because chokidar reports such a save as a
  change.
- **A deleted directory is not followed.** It is reported as itself, and no glob matches a
  directory. The result is refreshed only where the watcher also reports the files below it. On
  macOS it does for the files that are modules. This is not confirmed for the other backends.
- **The matchers of a deleted module stay.** If a later file matches one of them, the hook returns
  the id of a module that cannot be fetched again. The engine still knows the id, because partial
  scans do not prune `module_idx_by_abs_path`. Vite keeps its entries too, but looks each one up in
  its module graph.
- **The walk and the matcher can disagree.** The walk prunes every dot entry and `node_modules`
  below its root. The matcher keeps wildcards from matching a dot entry, and ignores `node_modules`
  anywhere in the path. So a glob in a module below `node_modules` never matches in the hook, like
  in vite, and a dot directory the glob names after a wildcard (`./*/.cache/*.js`) matches in the
  hook only. `fast_glob` and globstar can also read an unusual pattern differently.
- **`caseSensitive: false` folds differently on the two sides.** `fast_glob` has no nocase flag, so
  the walk lowercases the glob and the path. The matcher uses globstar's `case_insensitive`, which
  folds ASCII only. Character-class ranges (`[A-Z]`) can diverge from picomatch's `nocase` in the
  walk.
- **Paths are compared as strings.** The walk follows symbolic links and keeps the path it walked,
  while FSEvents reports canonical paths, so a file created below a linked directory is missed on
  macOS. On a disk that ignores case, a glob written in another case than the directory is missed
  the same way.
- **`watch.include` applies to the walk root.** It is matched as a path like any watch file, so an
  `include` that does not match the directory keeps it from being watched.
- **The watch set only grows.** The dev coordinator never unwatches, so a directory that stops being
  covered by any glob stays watched for the life of the server. This is the watch-mode behaviour (see
  `../watch-mode/implementation.md`), not something this feature adds.

## Related

- [implementation.md](./implementation.md): the machinery that realizes this
- `../dev-engine/implementation.md`: where `hotUpdate` runs in the HMR stage
- `../watch-mode/implementation.md`: how watch files reach the fs watcher, and what it reports
- [rolldown#10059](https://github.com/rolldown/rolldown/issues/10059): the feature request
- [rolldown#10019](https://github.com/rolldown/rolldown/issues/10019): full bundle mode phase 2
