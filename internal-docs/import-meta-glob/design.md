# `import.meta.glob` Design & Principles

## Summary

`crates/rolldown_plugin_vite_import_glob` expands each `import.meta.glob(...)` call into an object
of imports at `transform` time. That object is a snapshot of the filesystem. In dev mode it has to
be refreshed when a matching file appears or disappears. Otherwise a new route, locale or content
file stays invisible until the server restarts.

This doc records why the refresh works the way it does. The machinery is in
[implementation.md](./implementation.md).

The refresh runs through the `hotUpdate` hook, which is off by default. See
[Unresolved questions](#unresolved-questions).

## Where the responsibility sits

Unbundled Vite gets most of this for free. chokidar watches the project root, and the JS plugin
(`packages/vite/src/node/plugins/importMetaGlob.ts`) only answers "which modules care about this
file?" from its `hotUpdate` hook.

Under Vite's bundled dev (full bundle mode) neither half is free:

- `importGlobPlugin`'s `applyToEnvironment` returns the native plugin for the bundled environment,
  so the JS plugin and its `hotUpdate` hook are not used there.
- Vite's chokidar stops driving HMR. `hmr.ts` returns early on `config.experimental.bundledDev`, and
  file events come from rolldown's own watcher. That watcher only watches the files of the module
  graph and what plugins add with `addWatchFile`. A file that is not in the graph yet is reported
  only if a watched directory contains it.

So the native plugin owns both halves. It must put the directory it reads into the watch set, and it
must implement `hotUpdate`. See [rolldown#10059](https://github.com/rolldown/rolldown/issues/10059).

## Principles

1. **The hook asks what vite's hook asks.** `GlobMatcher` mirrors the matcher of vite's
   `vite:import-glob` plugin, with globstar in place of picomatch. It holds one union of the
   affirmed globs and one of the negated globs, all absolute. `exhaustive` turns `dot` on,
   `caseSensitive: false` turns case folding on, and `**/node_modules/**` is excluded unless
   `exhaustive` is set. A created or deleted file concerns a module if one of the module's matchers
   matches the file.

   One input differs from vite. With only negated globs, vite's matcher matches every file
   (`affirmed.length === 0 || affirmedMatcher(file)`). Rolldown returns before the walk for that
   input and records no matcher.

2. **The matcher is a second predicate next to the walk, and the only state.** The walk matches with
   `fast_glob` on a split path, and skips dot entries and `node_modules` while it walks. The matcher
   matches with globstar on the whole path. Vite has the same pair, tinyglobby for the walk and
   picomatch for the hook. Where the two disagree, a module is updated although its output did not
   change, or an update is missed until the module is transformed again.

   The hook does not remember which files the last walk matched. So it cannot tell a new file from
   one that was already in the result, and it does not know what was below a directory that is
   moved away. Both are listed under [Unresolved questions](#unresolved-questions).

3. **Watch the walk root, once.** The watcher watches a directory with everything below it, and it
   reports only files. A directory that appears is reported as the files inside it. So the walk root
   covers every directory the walk reads, including the ones created later.

   The watch also covers what the walk skips below the root, dot entries and `node_modules`. The
   matcher rejects their events. A `watch.exclude` of `dev()` that covers them keeps the watcher out
   of them. See `../watch-mode/implementation.md`.

4. **A missing walk root is watched through its topmost missing directory.** The walk finds nothing
   in a directory that does not exist. Without a watch, `import.meta.glob('./pages/*.vue')` would
   stay empty after `pages/` is created, which is a normal scaffolding flow.

   Waiting for a missing path is the watcher's job, not the plugin's. notify's `TrackPath` waits
   for it through its parent, and the parent must exist
   ([rolldown/notify#32](https://github.com/rolldown/notify/issues/32)). So for `./a/b/*.js` with
   `a/` missing, the plugin registers `a/`. Until `a/` appears, the watcher waits on `.` and does
   not watch what is below `.`. Then it watches `a/` with everything below it.

   The plugin does not register `.` itself. A registered directory is watched with everything below
   it, and [the watch set only grows](#unresolved-questions). So `.` would stay watched as a whole
   after `a/` is created.

5. **Only dev mode pays for any of this.** `hotUpdate` is a dev-only hook. Outside dev mode the
   matchers and the watched directory would be pure overhead, and the directory would show up in
   `this.getWatchFiles()` of ordinary builds. Both are gated on `options().is_dev_mode_enabled()`.

6. **The matchers are kept per module, never reset globally.** `buildStart` looks like the place for
   a reset (vite clears its map there), but rolldown calls `buildStart` for every scan, including
   the partial scans that transform only the changed modules again. A reset would drop the matchers
   of every module the scan left alone. `transform` runs on every fetch, so replacing and removing
   per module is accurate, and it also covers a user deleting the `import.meta.glob` call. A module
   deleted from disk never reaches `transform` again, so its matchers stay in the table.

7. **The hook adds to the affected set, it does not replace it.** Same as vite's
   `[...oldModules, ...modules]`. A file can be both a glob match and a module of its own. The glob
   owner joining the update must not push the file's own update out of it.

## Unresolved questions

- **The `hotUpdate` hook is off by default.** The `hotUpdate` option of `dev()` and
  `experimental.devMode.hotUpdate` stay off until file-to-module invalidation is complete
  ([rolldown#10714](https://github.com/rolldown/rolldown/pull/10714)), and Vite does not turn them
  on yet ([vite#22956](https://github.com/vitejs/vite/pull/22956)). Until then a new file does not
  reach the glob under Vite. A deleted file does, through the importers of its own module. The
  playground turns the hook on with `experimental.devMode.hotUpdate`.
- **`rolldown --watch` is not covered.** `hotUpdate` only runs in the dev engine, so a plain watch
  build still misses new glob matches.
- **A file saved by rename is reported as created.** An editor that writes a temporary file and
  renames it over the target makes the watcher report `Create` for a file that is already in the
  result. The matcher matches it, so the module of the glob is updated on every such save although
  its output is the same. A module the hook selects always ships, because it is exempt from the
  unchanged-output suppression. Vite does not see this, because chokidar reports such a save as a
  change.
- **A directory that is moved away is not followed.** The watcher reports the directory itself, and
  no glob matches a directory. The result is refreshed only where the watcher also reports the files
  below it. On macOS it does for the files that are modules. The other backends are not checked.
- **The matchers of a deleted module stay.** If a later file matches one of them, the hook returns
  the id of a module that cannot be fetched again. The engine still knows the id, because partial
  scans do not prune `module_idx_by_abs_path`. Vite keeps its entries too, but looks each one up in
  its module graph.
- **The walk and the matcher can disagree.** The walk skips every dot entry and `node_modules` below
  its root. The matcher only keeps wildcards from matching a dot entry, and rejects `node_modules`
  anywhere in the path. So a glob in a module below `node_modules` never matches in the hook, like
  in vite. A dot directory that the glob names after a wildcard (`./*/.cache/*.js`) matches in the
  hook only. `fast_glob` and globstar can also read an unusual pattern differently.
- **`caseSensitive: false` folds case differently on the two sides.** `fast_glob` has no nocase
  flag, so the walk lowercases the glob and the path. The matcher uses globstar's
  `case_insensitive`, which folds ASCII only. Character-class ranges (`[A-Z]`) in the walk can
  diverge from picomatch's `nocase`.
- **Paths are compared as strings.** The walk follows a linked directory and keeps the path through
  the link, while FSEvents reports canonical paths. So a file created below a linked directory is
  missed on macOS. On a disk that ignores case, a glob written in another case than the directory is
  missed the same way.
- **`watch.include` of `dev()` applies to the walk root.** It is matched like any watch file, so an
  `include` that does not match the directory keeps it from being watched.
- **The watch set only grows.** The dev coordinator never unwatches, so a directory that no glob
  covers anymore stays watched for the life of the server. This is how watch mode works (see
  `../watch-mode/implementation.md`), not something this feature adds.

## Related

- [implementation.md](./implementation.md): the machinery that realizes this
- `../dev-engine/implementation.md`: where `hotUpdate` runs in the HMR stage
- `../watch-mode/implementation.md`: how watch files reach the fs watcher, and what it reports
- [rolldown#10059](https://github.com/rolldown/rolldown/issues/10059): the feature request
- [rolldown#10019](https://github.com/rolldown/rolldown/issues/10019): full bundle mode phase 2
