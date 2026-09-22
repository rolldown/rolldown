# Module Side Effects — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

`normalize_side_effects` in `crates/rolldown/src/ecmascript/ecma_module_view_factory.rs` applies
the ranked sources. It reads a hook override first, then falls back to `lazy_check_side_effects`,
which consults `resolved_id.package_json` and finally the module's statements. So a module's
verdict is only as good as the `package_json` its `ResolvedId` carries.

## Where the metadata comes from

`resolve_id_with_plugins` in `crates/rolldown_plugin/src/utils/` builds every `ResolvedId`, by
one of two paths:

| Path                             | Source of the manifests                                                |
| -------------------------------- | ---------------------------------------------------------------------- |
| No plugin resolved the specifier | `Resolver::resolve`, via `oxc_resolver`'s own two lookups              |
| A plugin resolved it             | The hook's `package_json_path`, else `Resolver::resolve_absolute_path` |

The second row's fallback is what keeps design principle 2. A plugin that returns a bare id
string supplies no `package_json_path`, so `resolved_id_from_hook_output` resolves the id itself,
as an import of that absolute path. It only does so for a `ModuleIdKind::Path` id; a virtual or
bare id has no package to find. An id that names no file on disk gets no manifest, because only a
`load` hook can produce such a module.

The lookup keeps design principle 3 in two ways. `Resolver::resolve_absolute_path` runs on a
resolver with every rewrite option cleared. `resolve.alias`, `tsconfig` paths, extension aliases,
and the `browser` field could move the lookup to another file. The hook's id would then carry
that file's manifests. And a hook can set `skip_package_json_lookup` to keep its id bare. The
Vite resolver does so for `legacyInconsistentCjsInterop`, which opts out of format inference.

The flag travels with the `ResolvedId`. A hook that returns `ctx.resolve`'s or `this.resolve()`'s
answer as its own, as `viteAliasPlugin` does, turns that `ResolvedId` back into a hook output, and
`from_resolved_id` copies the flag. The callable form of the Vite resolver hands the Rust hook's
output straight to JS, and `BindingHookJsResolveIdOutput` carries the flag too. Vite's own JS
resolver omits `packageJsonPath` for the same option, so it has to set `skipPackageJsonLookup`.

### Two manifests, not one

Node.js answers `"type"` and `sideEffects` from different files. `oxc_resolver` attaches both to
every resolution, so both rows of the table above get the same pair:

| Field          | Rule                                                              | Lookup                                                                  |
| -------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `module_type`  | The nearest manifest, for `.js` and `.ts` only; decides `"type"`  | `oxc_resolver::esm_file_format`, which is Node's `LOOKUP_PACKAGE_SCOPE` |
| `package_json` | The package root inside `node_modules`, the nearest one elsewhere | `oxc_resolver::find_package_json_for_a_package`                         |

Inside `node_modules` the two can be different files, because a package root owns the
`sideEffects` globs and `"type"` has no such rule. The wrong manifest makes an aliased importer
use Node interop where direct resolution uses Babel interop. That changes what
`import v from 'cjs'` evaluates to at runtime.

`infer_module_def_format` in `crates/rolldown_resolver/src/resolver.rs` turns the pair into a
`ModuleDefFormat`: `module_type` first, then `package_json#type` for every js-like extension.
Rolldown treats `.jsx` and `.tsx` like `.js` and `.ts` here, and `oxc_resolver` does not.

Recovery does not copy these rules. A hand-written walk has to reproduce both lookups and the
extension split between them. A walk that reads one manifest inverts either `.js` or `.jsx`.
One lookup shared by both paths cannot disagree with itself. That is the bug this page exists to
prevent.

## Regression test

`crates/rolldown/tests/rolldown/issues/10909/` holds one test per manifest. Both use a plugin
that returns a bare id string, and one importer, so neither depends on task completion order.

`plugin_resolved_id_keeps_package_side_effects_policy` pins which manifest owns `sideEffects`,
by giving each candidate manifest a different answer. The root declares `sideEffects: false`,
`dep` declares `sideEffects: ["lib/effectful.js"]`, and `dep/lib` declares none:

| Manifest used                             | `nested.js`        | `effectful.js` |
| ----------------------------------------- | ------------------ | -------------- |
| `dep/lib` — the nearest, no `sideEffects` | kept by analysis ✗ | kept           |
| `dep` — correct                           | dropped            | kept           |
| the fixture root — beyond `node_modules`  | dropped            | dropped ✗      |

Asserting that `nested.js` goes and `effectful.js` stays therefore holds only for the middle row.
Give the two packages the same answer and the test stops telling the rows apart.

`plugin_resolved_id_keeps_package_type_precedence` pins the other rule. `fmt` declares
`"type": "module"` and `fmt/lib` declares `"type": "commonjs"`. The test bundles the same
importer twice, once resolved directly and once through the plugin, for `.js` and for `.jsx`.
Those two take opposite branches of the precedence above, so the pair fails if recovery settles
on either manifest alone.

`plugin_resolved_id_reads_metadata_from_the_returned_path` pins principle 3 against rewrites. The
plugin returns `alias-src/index.js` as a ready absolute path, and `resolve.alias` maps that path
to `alias-target/index.js`. The target's package declares `sideEffects: false`, so a lookup that
follows the alias drops the source module's side effect.

`plugin_resolved_id_can_skip_package_json_lookup` pins the opt-out. The `.jsx` importer takes its
format from `fmt`'s `"type": "module"`, which selects Node interop. With the flag set, the
importer has no format and keeps Babel interop.

The forwarding coverage lives with the flag itself. `plugin/skip_package_json_lookup` in the Rust
tests checks that the opt-out reaches the `ResolvedId` from `ctx.resolve` and survives
`from_resolved_id`. Two Node fixtures cover the JS side.
`plugin/context/resolve-skip-package-json-lookup` reads the flag off a forwarded `this.resolve()`
answer, and `builtin-plugin/vite-resolve-legacy-cjs-interop` reads it off the callable Vite
resolver.

`plugin_resolved_id_reads_metadata_from_the_returned_path` pins principle 3 against rewrites. The
plugin returns `alias-src/index.js` as a ready absolute path, and `resolve.alias` maps that path
to `alias-target/index.js`. The target's package declares `sideEffects: false`, so a lookup that
follows the alias drops the source module's side effect.

`plugin_resolved_id_can_skip_package_json_lookup` pins the opt-out. The `.jsx` importer takes its
format from `fmt`'s `"type": "module"`, which selects Node interop. With the flag set, the
importer has no format and keeps Babel interop.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- `../module-id/implementation.md` — what an id is, and when it is a real path
