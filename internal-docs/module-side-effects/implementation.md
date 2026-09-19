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

| Path                             | Source of the manifests                                         |
| -------------------------------- | --------------------------------------------------------------- |
| No plugin resolved the specifier | `Resolver::resolve`, via `oxc_resolver`'s own two lookups       |
| A plugin resolved it             | The hook's `package_json_path`, else `Resolver::package_scopes` |

The second row's fallback is what keeps design principle 2. A plugin that returns a bare id
string supplies no `package_json_path`, so `resolved_id_from_hook_output` looks the manifests up
from the id itself. It only does so for a `ModuleIdKind::Path` id; a virtual or bare id has no
package to find.

### Two manifests, not one

Node.js answers `"type"` and `sideEffects` from different files, so `PackageScopes` carries both
and `Resolver::package_scopes` fills them in one walk:

| Field                | Rule                                                              | Mirrors                                                                 |
| -------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `nearest`            | The nearest manifest; decides `"type"` and so the module format   | `oxc_resolver::esm_file_format`, which is Node's `LOOKUP_PACKAGE_SCOPE` |
| `side_effects_owner` | The package root inside `node_modules`, the nearest one elsewhere | `oxc_resolver::find_package_json_for_a_package`                         |

Inside `node_modules` the two can be different files, because `sideEffects` globs are written
relative to a package root while `"type"` is not. Picking the wrong one makes an aliased importer
use Node interop where direct resolution uses Babel interop, which changes what
`import v from 'cjs'` evaluates to at runtime.

`"type"` needs both fields, because normal resolution consults them in order:
`oxc_resolver` reads the nearest manifest, but only for `.js` and `.ts`, and rolldown then falls
back to the owning manifest for every js-like extension. `recovered_module_def_format` applies the
same order. Using `nearest` alone inverts `.jsx` and `.tsx`; using `side_effects_owner` alone
inverts `.js` and `.ts`.

Each rule has to match the one `oxc_resolver` applies when it resolves the same file itself.
Where they disagree, one file gets two answers depending on the specifier that reached it, which
is the bug this whole page exists to prevent.

The result caches by the module's parent directory, so one walk serves every module in it.

## Regression test

`crates/rolldown/tests/rolldown/issues/10909/` holds one test per manifest. Both use a plugin
that returns a bare id string, and one importer, so neither depends on task completion order.

`plugin_resolved_id_keeps_package_side_effects_policy` pins where the `sideEffects` walk stops,
by giving each candidate manifest a different answer. The root declares `sideEffects: false`,
`dep` declares `sideEffects: ["lib/effectful.js"]`, and `dep/lib` declares none:

| Walk stops at                              | `nested.js`        | `effectful.js` |
| ------------------------------------------ | ------------------ | -------------- |
| `dep/lib` — too early, no `sideEffects`    | kept by analysis ✗ | kept           |
| `dep` — correct                            | dropped            | kept           |
| the fixture root — ran past `node_modules` | dropped            | dropped ✗      |

Asserting that `nested.js` goes and `effectful.js` stays therefore holds only for the middle row.
Give the two packages the same answer and the test stops telling the rows apart.

`plugin_resolved_id_keeps_package_type_precedence` pins the other rule. `fmt` declares
`"type": "module"` and `fmt/lib` declares `"type": "commonjs"`, and the same importer is bundled
twice — once resolved directly, once through the plugin — for a `.js` and a `.jsx` extension.
Those two take opposite branches of the precedence above, so the pair fails if recovery settles
on either manifest alone.

Two gaps remain, both unchanged from before the recovery existed, so neither is a regression:
`oxc_resolver`'s type lookup crosses the `node_modules` and `@scope` boundaries while this walk
stops at them. A dependency whose only manifest sits beyond that boundary therefore resolves as
`EsmPackageJson` directly and `Unknown` through recovery. Closing that would change which
manifests may govern a dependency, which is a separate decision.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- `../module-id/implementation.md` — what an id is, and when it is a real path
