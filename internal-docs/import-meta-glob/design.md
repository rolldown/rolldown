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

1. **A created file is judged the way the walk judges it.** `GlobMatcher` stores the inputs the
   walk used (walk root, the `(static prefix, pattern)` split, `exhaustive`, `caseSensitive`) and
   decides with the same `fast_glob` call and the same pruning rules. A predicate of its own would
   drift, and both directions are bugs: a laxer hook runs modules again whose glob output cannot
   have changed, a stricter one silently drops updates. The unit tests split their globs with the
   splitter of the walk for the same reason.

   This is also why the hook does not copy vite's matcher. Vite treats "only negative patterns" as
   matching everything (`affirmed.length === 0 || affirmedMatcher(file)`), while rolldown's walk
   yields nothing for that input.

2. **The result of the last walk answers what the predicate cannot.** `GlobMatcher` also keeps the
   files the walk matched. Two events need them:
   - A deleted directory is reported as itself, because its files are unknown by then. No pattern
     matches a directory, but the result shows whether one of its files was below it.
   - A file saved by renaming a temporary file over it is reported as created. The result shows that
     it is already part of it, so nothing changes.

   Being exact matters here. A module the hook selects always ships, because it is exempt from the
   unchanged-output suppression.

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
   module deleted from disk never reaches `transform` again, so the hook forgets it when it sees
   the deletion.

7. **The hook adds to the affected set, it does not replace it.** Same as vite's
   `[...oldModules, ...modules]`. A file can be both a glob match and a module in its own right. The
   glob owner joining the update must not push the file's own update out of it.

## Unresolved questions

- **The `hotUpdate` hook is off by default.** `dev.hotUpdate` stays off until file-to-module
  invalidation is complete ([rolldown#10714](https://github.com/rolldown/rolldown/pull/10714)), and
  Vite does not pass the option yet ([vite#22956](https://github.com/vitejs/vite/pull/22956)). Until
  then a new file does not reach the glob under Vite, and the playground is skipped. A deleted file
  does, through the importers of its own module.
- **`rolldown --watch` is not covered.** `hotUpdate` only runs in the dev engine, so a plain watch
  build still misses new glob matches.
- **`caseSensitive: false` is ASCII-only**, in the hook exactly as in the walk: `fast_glob` has no
  nocase flag, so both sides are lowercased. Character-class ranges (`[A-Z]`) and non-ASCII case
  folding can diverge from picomatch's `nocase`.
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
