# `import.meta.glob` — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

`crates/rolldown_plugin_vite_import_glob` does two jobs. `transform` rewrites each
`import.meta.glob(...)` call into a literal object of imports by walking the filesystem, and — in dev
mode only — the same walk feeds two side outputs that keep that rewrite fresh: the directories it read
go into the watch set, and a `GlobMatcher` per call is kept so `hotUpdate` can map a created or deleted
file back to the module that has to be re-transformed.

## Concept → file map

| Concept                                          | Location                                                               |
| ------------------------------------------------ | ---------------------------------------------------------------------- |
| Plugin, `hotUpdate`                              | `src/lib.rs`                                                           |
| Predicate table ops, post-build predicate        | `src/matcher.rs`                                                       |
| AST visit, glob resolution, the walk             | `src/utils.rs`                                                         |
| `hotUpdate` chain over plugins                   | `crates/rolldown_plugin/src/plugin_driver/watch_hooks.rs`              |
| Where the chain runs in an HMR round             | `crates/rolldown/src/hmr/hmr_stage.rs`                                 |
| Watch files → fs watcher (dev)                   | `crates/rolldown_dev/src/bundle_coordinator.rs` (`update_watch_paths`) |

## Build-time walk (`utils.rs`)

`GlobImportVisit::eval_glob_expr` resolves every glob in one call to a `PathWithGlob`
— a `(static prefix, pattern)` pair where the prefix is a real directory path and the pattern keeps the
leading separator, so matching is `path.strip_prefix(prefix)` followed by `fast_glob::glob_match`.
`get_common_base` then reduces the positive prefixes to the directory `walkdir` is rooted at, and
`filter_entry` prunes dot entries and `node_modules` unless `exhaustive` is set.

Two fields on the visitor turn on the dev-only side outputs:

- `is_dev_mode` — `ctx.options().is_dev_mode_enabled()`, threaded in from `lib.rs`.
- `matchers` — one `GlobMatcher` per glob call, drained by `transform`.

Inside the walk loop, directory entries are registered with `PluginContext::add_watch_file` and skipped.
Note that `utils.rs` holds the _inner_ `PluginContext`, not the transform context, so this is a plain
watch registration: it does not add a transform dependency and does not skip `\0` virtual modules.
Registrations are non-recursive; `design.md` principle 2 explains why that is enough.

When the walk root does not exist, the visitor walks up `Path::ancestors` to the closest existing
directory and registers that instead (principle 3).

## Predicate (`matcher.rs`)

`GlobMatcher` holds the walk's inputs and `matches(file)` replays its decision in the same order the
walk reaches it:

1. `file` must live under `walk_root` — compared on separator boundaries, so `/a/bc.js` is not read as
   living inside `/a/b`. This is also the cheap early exit for the common case of an unrelated edit.
2. Unless `exhaustive`, no segment of `file` _relative to_ `walk_root` may start with `.` or be
   `node_modules`. Testing only the relative segments is what reproduces `filter_entry`'s `depth() == 0`
   exemption: a dot directory that is part of the glob's own root (`./.storybook/*.js`) is fine, one
   below it is not.
3. `!negated.any(rule) && positive.any(rule)`, with `rule` the same strip-prefix-then-`glob_match` pair
   the walk uses. `caseSensitive: false` lowercases both sides, matching the walk's approximation.

`touches_base(path)` is separate and answers a different question: is `path` one of the positive static
prefixes, or an ancestor of one? That is the signal for "a directory this glob is derived from came or
went", which is the only thing observable while the prefix itself does not exist.

`may_gain_matches_below(dir)` is the third predicate: `dir` is strictly inside the walk root, survives
the prune check, and some positive pattern reaches deeper than its own directory (a separator past
the anchoring one, or `**`). A brand-new directory matches no pattern and is not
watched yet, so this is what makes the round that sees its creation re-walk. It is only consulted for a
path that really is a directory — see `design.md` principle 2 for why the gate matters.

## Predicate table and `hotUpdate` (`lib.rs`, `matcher.rs`)

`glob_matchers: FxDashMap<ArcStr, Vec<GlobMatcher>>` is keyed by slash-normalized module id. The plugin
instance lives in `PluginDriverFactory.plugins` for the bundler's lifetime, so the table survives
incremental rebuilds even though a fresh `PluginDriver` (and a fresh `watch_files` set) is created per
build.

`transform` keeps it honest per module rather than resetting it globally — `set_globs` upserts (and
removes when a module's matchers come back empty), `remove_globs` drops a module that no longer
contains a glob or no longer parses. Both early-return paths call `remove_globs`, guarded by
`!glob_matchers.is_empty()` so projects without globs never pay for id normalization. Principle 5 in
`design.md` explains why `buildStart` is the wrong place.

`hot_update`:

- Declines `WatcherChangeKind::Update` outright, like vite: a content edit cannot change a glob's
  result _set_, and the engine's default mapping already covers the file's own module.
- Collects every module id whose matchers report `matches(file) || touches_base(file)`, skipping the id
  that equals `file` itself (the walk excludes a glob's own module to avoid a self-import; the hook must
  not put it back).
- Falls back to `may_gain_matches_below(file)` only when those miss and `file` is a directory. The
  `is_dir` call is resolved lazily and at most once per round.
- Returns `None` when nothing matched — declining leaves the engine's default set untouched, which is
  what makes a stray non-matching file in a watched directory end the round as a noop.
- Otherwise returns `args.modules` with the matched ids appended, deduplicated. Appending rather than
  replacing mirrors vite's `[...oldModules, ...modules]`.

`register_hook_usage` therefore reports `Transform | HotUpdate`.

## One round end to end

Creating `pages/c.js` under `import.meta.glob('./pages/*.js')` in `main.js`:

1. `pages/` is in the fs watcher because the previous build's walk registered it. The create event
   reaches `BundleCoordinator::handle_watch_event`, which does not filter by watch set, and is queued as
   an `Hmr` task.
2. `HmrStage::compute_hmr_update_for_file_changes` computes the default affected set for
   `…/pages/c.js` — empty, since no module and no transform dependency point at it — and runs the
   `hotUpdate` chain anyway.
3. The plugin's matcher for `main.js` matches, so the hook returns `[main.js]`. Hook-returned modules are
   exempt from the unchanged-output suppression, so the update ships even though `main.js`'s source is
   byte-identical.
4. `main.js` is re-fetched: `transform` walks `pages/` again, emits the object with `./pages/c.js` in it,
   and re-registers the directories (all already known).
5. The partial scan pulls `pages/c.js` into the graph and the patch ships. `main.js` self-accepts, so the
   client re-runs it in place.

Deleting a file is the same, except step 2's default set contains the deleted module itself and the
engine expands to its importers.

## Tests

- `src/matcher.rs` unit tests cover the predicate: single-level vs `**` patterns, separator-boundary
  rejection, negated patterns, the positive-hit requirement, dot / `node_modules` pruning,
  `exhaustive`, case folding, `touches_base`, and `may_gain_matches_below`.
- `packages/rolldown/tests/fixtures/builtin-plugin/import-glob/*` are the build-time snapshots. They must
  not move: none of this changes what `transform` emits.
- `packages/test-dev-server/tests/playground/hmr-import-glob` is the end-to-end check (add, delete,
  non-matching file, new nested directory, directory missing at boot). It runs on the browser platform,
  where vite installs the native plugin itself, so it exercises the real integration. Running it locally
  needs `just setup-vite`.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- `../dev-engine/implementation.md` — the HMR round this hooks into
- `../watch-mode/implementation.md` — watch-file registration and its known gaps
