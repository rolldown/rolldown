# `import.meta.glob` Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

`crates/rolldown_plugin_vite_import_glob` rewrites each `import.meta.glob(...)` call into an object
of imports in `transform`, by walking the filesystem. In dev mode each call also leaves two things
behind that keep the object fresh. The walk root goes into the watch set, and a `GlobMatcher` is
kept so `hotUpdate` can map a created or deleted file back to the module that has to be transformed
again.

## Concept to file map

| Concept                              | Location                                                               |
| ------------------------------------ | ---------------------------------------------------------------------- |
| Plugin, `transform`, `hotUpdate`     | `src/lib.rs`                                                           |
| Matcher and its table                | `src/matcher.rs`                                                       |
| AST visit, glob resolution, the walk | `src/utils.rs`                                                         |
| `hotUpdate` chain over plugins       | `crates/rolldown_plugin/src/plugin_driver/watch_hooks.rs`              |
| Where the chain runs in an HMR round | `crates/rolldown/src/hmr/hmr_stage.rs`                                 |
| Watch files to fs watcher (dev)      | `crates/rolldown_dev/src/bundle_coordinator.rs` (`update_watch_paths`) |
| What the fs watcher reports          | `crates/rolldown_fs_watcher/src/notify/event_map.rs`                   |

## Build-time walk (`utils.rs`)

The `transform` hook runs in three steps. It copies each `import.meta.glob` call out of the AST
(`collect_glob_call`), awaits the resolve of the globs that need it, like `$lib/*.js`
(`resolve_globs`), and then walks the filesystem for each call (`GlobImportVisit::transform`).

`eval_glob_expr` turns every glob of a call into a `PathWithGlob`, a `(path, glob)` pair. `path` is
the literal part, up to the last `/` before the first glob syntax. `glob` is the rest, so `path` and
`glob` joined give the absolute glob again. The `/` between them goes with `glob` (`/p` and
`/**/x.js`), except when the glob as written starts with the pattern (`/p/` and `**/x.js`). The walk
matches a file with `strip_prefix(path)` and then `fast_glob::glob_match(glob, rest)`.

`get_common_base` reduces the `path`s of the positive globs to the directory the walk starts at.
`walk_files` skips dot entries and `node_modules` below it unless `exhaustive` is set. A linked
directory gets its own walk that starts at the link, so its files keep the path through the link.

A call with only negated globs returns before the walk. Nothing can match, and the common base of no
`path`s would be the whole root.

Both dev outputs are gated on the visitor's `is_dev_mode`, which `lib.rs` sets from
`ctx.options().is_dev_mode_enabled()`.

`watch_walk_root` registers the walk root with `add_watch_file`, or its topmost missing directory if
it does not exist (`design.md` principle 4). A walk root that is not absolute is not registered. The
visitor holds the plain `PluginContext`, not the transform context, so this only adds a watch file.
It does not add a transform dependency, which would put the module into the affected set of every
change below the directory.

The matcher is built before the walk, from the same `PathWithGlob`s, and pushed to the visitor's
`matchers`. `PathWithGlob::to_absolute_glob` joins `path` and `glob` and escapes the glob syntax in
`path`, like vite's `globSafeResolvedPath`, so a directory named `[lang]` stays literal. A glob that
globstar does not compile leaves the call without a matcher.

## Matcher (`matcher.rs`)

`GlobMatcher` is two compiled globstar globs:

- `affirmed` is the union of the positive globs.
- `negated` is the union of the negated globs, plus `**/node_modules/**` unless `exhaustive` is
  set. It is `None` when that leaves nothing.

Both are compiled with `dot = exhaustive` and `case_insensitive = !caseSensitive`. Vite passes the
same to picomatch as `dot`, `nocase` and `ignore`.

`is_match(file)` is `affirmed.is_match(file) && !negated.is_match(file)`, on the slash-normalized
absolute path the hook receives. With `dot` off, a wildcard does not match a segment that starts
with `.`, while a dot directory the glob names (`./.storybook/*.js`) matches as written.

## Matcher table and `hotUpdate` (`lib.rs`, `matcher.rs`)

`glob_matchers: FxDashMap<ArcStr, Vec<GlobMatcher>>` is keyed by slash-normalized module id. The
plugin instance lives in `PluginDriverFactory` for the lifetime of the bundler, so the table
survives incremental rebuilds, although each build gets a fresh `PluginDriver` and a fresh
`watch_files` set.

In dev mode `transform` ends with `set_globs`. It replaces the matchers of the module, or removes
them when there are none, because the glob call is gone or the module is not JavaScript. A transform
that fails returns before it and leaves them as they were. Outside dev mode the table is never
touched. Principle 6 in `design.md` explains why `buildStart` is the wrong place.

`hot_update` does this:

- It declines `Update`, like vite. A content edit cannot change which files a glob matches, and the
  engine's default set already holds the file's own module.
- It collects every module id with a matcher that matches `args.file`.
- It returns `None` when nothing is collected. Declining leaves the engine's default set untouched,
  which is what makes a stray file in a watched directory end the round as a noop.
- Otherwise it returns `args.modules` with the collected ids appended. The ids are sorted first,
  because the table has no stable order. The engine drops an id it does not know, and its set
  ignores one it already has.

`register_hook_usage` reports `Transform | HotUpdate`.

## One round end to end

Creating `pages/c.js` under `import.meta.glob('./pages/*.js')` in `main.js`, with the `hotUpdate`
hook turned on:

1. `pages/` is in the fs watcher, because the previous build registered it. The watcher reports
   `Create` for `pages/c.js`, and `BundleCoordinator::handle_watch_event` queues an `Hmr` task.
2. `HmrStage::compute_hmr_update_for_file_changes` computes the default affected set for
   `…/pages/c.js`. It is empty, since no module and no transform dependency point at the file. The
   `hotUpdate` chain runs anyway.
3. The matcher of `main.js` matches the file, so the hook returns `[main.js]`. Modules returned by a
   hook are exempt from the unchanged-output suppression.
4. `main.js` is fetched again. `transform` walks `pages/`, emits the object with `./pages/c.js` in
   it, and replaces the matchers.
5. The partial scan pulls `pages/c.js` into the graph, and the patch ships. `main.js` accepts
   itself, so the client runs it again in place.

Deleting a file goes the same way, except that the default set of step 2 holds the deleted module
itself, and the engine starts the update from its importers.

## Tests

- The unit tests in `src/matcher.rs` cover the matcher, the escaping in
  `PathWithGlob::to_absolute_glob`, and the hook.
- `packages/rolldown/tests/fixtures/builtin-plugin/import-glob/*` are the build-time snapshots.
  They must not move, because none of this changes what `transform` emits.
- `packages/test-dev-server/tests/playground/hmr-import-glob` is the end-to-end check. It covers a
  new file, a deleted file, a file that does not match, a new nested directory, and a directory
  missing at boot, with and without its parent. It runs on the browser platform, where vite installs
  the native plugin itself, and it turns the hook on with `experimental.devMode.hotUpdate`. Running
  it locally needs `just setup-vite`.

## Related

- [design.md](./design.md): the principles and trade-offs behind this
- `../dev-engine/implementation.md`: the HMR round this hooks into
- `../watch-mode/implementation.md`: watch-file registration and event mapping
