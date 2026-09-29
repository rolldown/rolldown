# `import.meta.glob` — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

`crates/rolldown_plugin_vite_import_glob` does two jobs. `transform` rewrites each
`import.meta.glob(...)` call into a literal object of imports by walking the filesystem. In dev mode
only, the same walk has two side outputs that keep that rewrite fresh: the walk root goes into the
watch set, and a `GlobMatcher` per call is kept so `hotUpdate` can map a created or deleted path back
to the module that has to be transformed again.

## Concept → file map

| Concept                              | Location                                                               |
| ------------------------------------ | ---------------------------------------------------------------------- |
| Plugin, `transform`, `hotUpdate`     | `src/lib.rs`                                                           |
| Matchers, their table, the owners    | `src/matcher.rs`                                                       |
| AST visit, glob resolution, the walk | `src/utils.rs`                                                         |
| `hotUpdate` chain over plugins       | `crates/rolldown_plugin/src/plugin_driver/watch_hooks.rs`              |
| Where the chain runs in an HMR round | `crates/rolldown/src/hmr/hmr_stage.rs`                                 |
| Watch files → fs watcher (dev)       | `crates/rolldown_dev/src/bundle_coordinator.rs` (`update_watch_paths`) |
| What the fs watcher reports          | `crates/rolldown_fs_watcher/src/notify/event_map.rs`                   |

## Build-time walk (`utils.rs`)

`GlobImportVisit::eval_glob_expr` resolves every glob in one call to a `PathWithGlob`, a
`(static prefix, pattern)` pair where the prefix is the path up to the first segment with glob
syntax and the pattern is the rest, so matching is `path.strip_prefix(prefix)` followed by
`fast_glob::glob_match`. The separator between the two is on the side of the pattern, unless the
glob as written starts with the pattern (`**/index.js`).
`get_common_base` then reduces the positive prefixes to the directory `walkdir` is rooted at, and
`filter_entry` prunes dot entries and `node_modules` unless `exhaustive` is set.

A call with only negated globs returns before the walk. Nothing can match, and the common base of no
prefixes is the whole root.

Two fields on the visitor turn on the side outputs of dev mode:

- `is_dev_mode`: `ctx.options().is_dev_mode_enabled()`, passed in from `lib.rs`.
- `matchers`: one `GlobMatcher` per glob call, taken by `transform`.

`watch_walk_root` registers the walk root with `PluginContext::add_watch_file`, or its topmost
missing directory if it does not exist (`design.md` principle 4). A walk root that is not absolute
is not registered. `utils.rs` holds the inner
`PluginContext`, not the transform context, so this is a plain watch registration. It does not add a
transform dependency, which would put the module into the affected set of every change below the
directory.

The walk collects the files it matched, and the matcher is built from them after the loop.

## Matcher (`matcher.rs`)

`GlobMatcher` holds the inputs of the walk and its result. It answers two questions, one per event
kind:

- `gains(file)`: would creating `file` add it to the result? The walk must match it, and it must not
  be part of the result already.
- `loses(path)`: would deleting `path` remove files from the result? `path` must be a file of the
  result or a directory above one.

`matches(file)` replays the decision of the walk in the order the walk reaches it:

1. `file` must be `walk_root` or live under it, compared on separator boundaries, so `/a/bc.js` is
   not read as living inside `/a/b`. This is also the cheap early exit for an unrelated path.
2. Unless `exhaustive`, no segment of `file` relative to `walk_root` may start with `.` or be
   `node_modules`. Testing only the relative segments reproduces the `depth() == 0` exemption of
   `filter_entry`: a dot directory that is part of the glob's own root (`./.storybook/*.js`) is fine,
   one below it is not.
3. `!negated.any(rule) && positive.any(rule)`, with `rule` the same pair of `strip_prefix` and
   `glob_match` the walk uses. `caseSensitive: false` lowercases both sides, like the walk.

## Matcher table and `hotUpdate` (`lib.rs`, `matcher.rs`)

`glob_matchers: FxDashMap<ArcStr, Vec<GlobMatcher>>` is keyed by slash-normalized module id. The
plugin instance lives in `PluginDriverFactory.plugins` for the lifetime of the bundler, so the table
survives incremental rebuilds even though a fresh `PluginDriver` (and a fresh `watch_files` set) is
created per build.

`transform` maintains it per module. In dev mode it ends with `set_globs`, which replaces the
matchers of the module, or removes them when there are none: the glob call is gone, or the module
is not JavaScript. A transform that fails leaves them as they were. Outside dev mode the table is
never touched. Principle 6 in `design.md` explains why `buildStart` is the wrong place.

`hot_update` returns `add_glob_owners`, which:

- Picks the question by event kind: `gains` for `Create`, `loses` for `Delete`. It declines `Update`
  like vite: a content edit cannot change which files a glob matches, and the default mapping of the
  engine already covers the file's own module.
- For `Delete`, first forgets the modules that are `file` or live below it. They cannot be fetched
  again. The id is compared without its query.
- Collects every module id with a matcher that says yes. It skips the module that is `file`, which
  the walk leaves out too, and the modules that are in `args.modules` already.
- Returns `None` when nothing is collected. Declining leaves the default set of the engine
  untouched, which is what makes a stray file in a watched directory end the round as a noop.
- Otherwise returns `args.modules` with the collected ids appended, sorted, because the table has no
  stable order.

`register_hook_usage` therefore reports `Transform | HotUpdate`.

## One round end to end

Creating `pages/c.js` under `import.meta.glob('./pages/*.js')` in `main.js`, with `dev.hotUpdate` on:

1. `pages/` is in the fs watcher because the walk of the previous build registered it. The watcher
   reports `Create` for `pages/c.js`, and `BundleCoordinator::handle_watch_event` queues an `Hmr`
   task.
2. `HmrStage::compute_hmr_update_for_file_changes` computes the default affected set for
   `…/pages/c.js`. It is empty, since no module and no transform dependency point at it. The
   `hotUpdate` chain runs anyway.
3. The matcher of `main.js` gains the file, so the hook returns `[main.js]`. Modules returned by a
   hook are exempt from the unchanged-output suppression.
4. `main.js` is fetched again: `transform` walks `pages/`, emits the object with `./pages/c.js` in it,
   and replaces the matchers.
5. The partial scan pulls `pages/c.js` into the graph and the patch ships. `main.js` accepts itself,
   so the client runs it again in place.

Deleting a file is the same, except that the default set of step 2 contains the deleted module
itself and the engine expands to its importers.

## Tests

- The unit tests in `src/matcher.rs` cover the matcher and `add_glob_owners`. They are the only
  tests of the hook that run while the playground is skipped. The globs are split by
  `PathWithGlob`, like in the walk.
- `packages/rolldown/tests/fixtures/builtin-plugin/import-glob/*` are the build-time snapshots. They
  must not move: none of this changes what `transform` emits.
- `packages/test-dev-server/tests/playground/hmr-import-glob` is the end-to-end check: add, delete, a
  file that does not match, a new nested directory, a directory moved away, a directory missing at
  boot, with and without its parent. It runs on the browser platform, where vite installs the native
  plugin itself. It is skipped while `dev.hotUpdate` is off by default. Running it locally needs
  `just setup-vite`.

## Related

- [design.md](./design.md): the principles and trade-offs behind this
- `../dev-engine/implementation.md`: the HMR round this hooks into
- `../watch-mode/implementation.md`: watch-file registration and event mapping
