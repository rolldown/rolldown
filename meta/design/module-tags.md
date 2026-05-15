# Module Tags

## Summary

Module tags attach a set of string labels to modules, allowing them to be identified and filtered in other processes — most importantly in manual code splitting groups. Tags can be user-defined (via entry config) or built-in (computed by rolldown based on the module graph).

## Motivation

Several real-world scenarios require knowing _how_ a module is loaded, not just _what_ it is:

1. **Splitting initial-loading code from async code** — webpack's `chunks: 'initial' | 'async'`. In large apps, hundreds or thousands of modules in main.js's static import tree create a render-blocking request waterfall. Capturing them into a manual code splitting group and using `maxSize` to split into parallel chunks reduces initial requests with zero over-fetch.

2. **Identifying static import chains** — VitePress needs to know which modules are statically reachable from an entry to create a framework chunk.

3. **Separating server and browser code** — Framer bundles server + browser entries in the same build. Chunking rules must avoid putting server-only code into browser chunks.

All three are solved by tagging modules with metadata about their loading context, then filtering on those tags in code splitting groups.

## Design

### Built-in tags

Built-in tags use a `$` prefix to distinguish them from user-defined tags. Names starting with `$` are reserved — user-defined tags must not use the `$` prefix.

- **`$initial`** — the module is statically imported by at least one user-defined entry point, or is part of its static dependency chain. These modules are render-blocking: the browser must fetch and execute them all before the entry can run. A module that is both statically and dynamically imported still gets `$initial`, since it's already a dependency of an initial entry regardless of the dynamic `import()`.
- **`$lazy`** — the module is reachable only via `import()` and is NOT in any entry's static import chain. `$initial` and `$lazy` are mutually exclusive.
- **`$lazy-entry:<identifier>`** — parameterized tag. The module is the target of a specific dynamic `import()` matching `<identifier>`, or is part of that target's static import chain. A module can carry both `$initial` and `$lazy-entry:X` simultaneously (e.g. shared between the static root tree and a lazy boundary); group priority determines which group actually captures it. `<identifier>` is resolved against the module graph at config parse time. See [`$lazy-entry` matching](#lazy-entry-matching).

### Using tags in manual code splitting

Groups can filter by tags using `tags`:

```js
output: {
  codeSplitting: {
    groups: [
      {
        name: 'initial-deps',
        tags: ['$initial'],
        maxSize: 1048576,
        priority: 10,
      },
    ],
  },
}
```

`tags` uses AND semantics: only modules that have **all** specified tags are captured by the group. It combines with other filters (`test`, `minShareCount`, etc.) — a module must match all criteria.

### `$lazy-entry` matching

`$lazy-entry:<identifier>` is a parameterized tag that selects a specific dynamic import target and tags that target plus its static-import closure. It has **two sub-features**, disambiguated by identifier syntax — they are independently motivated, independently shippable, and track different data, but share the same tag prefix, error semantics, priority behavior, and config surface.

- **Path form** — `$lazy-entry:./…` — matches against the **resolved filesystem path** of dynamic import targets. Marked by a leading `./`. Use for `import('./signup.module.ts')`.
- **Bare-specifier form** — `$lazy-entry:<bare>` — matches against the **bare specifier** used in dynamic `import()` calls. Use for `import('react')`.

Rolldown collects every `$lazy-entry:*` reference from `codeSplitting.groups` at config parse time, classifies each by the leading-`./` rule, resolves to exactly one dynamic import target, and applies the tag.

#### Sub-feature A — path form (`$lazy-entry:./…`)

The identifier starts with `./`. Rolldown strips the `./` and treats the remainder as a **path-segment-aligned suffix** of the resolved module path.

**Matching rule:** after stripping `./`, the identifier must equal a suffix of the resolved path, aligned on `/` boundaries. Extensions included — no stripping, no case-insensitive match.

| Identifier                    | Resolved path of target                           | Match? | Why                                        |
| ----------------------------- | ------------------------------------------------- | ------ | ------------------------------------------ |
| `./signup.module.ts`          | `/project/src/auth/signup.module.ts`              | ✅     | `signup.module.ts` is a `/`-aligned suffix |
| `./auth/signup.module.ts`     | `/project/src/auth/signup.module.ts`              | ✅     | multi-segment suffix                       |
| `./src/auth/signup.module.ts` | `/project/src/auth/signup.module.ts`              | ✅     | longer suffix, narrows ambiguity           |
| `./bar.ts`                    | `/project/src/bar.ts` (from `import('./bar.js')`) | ✅     | matches what's on disk, not the specifier  |
| `./bar.js`                    | `/project/src/bar.ts`                             | ❌     | resolved extension is `.ts`                |
| `./foo.js`                    | `/project/src/foo.ts`                             | ❌     | different extension                        |
| `./th/signup.module.ts`       | `/project/src/auth/signup.module.ts`              | ❌     | mid-segment — `th` isn't a full segment    |

**Rationale:** resolved paths carry absolute-path prefixes (`/Users/you/project/…`) that users don't care about — suffix match strips that noise. Segment alignment prevents silent substring matches like `signup.module.ts` accidentally matching `foosignup.module.ts`.

**Data tracking:** reuses existing resolved-path data. No new per-module metadata.

**Primary motivation:** route-scoped chunk grouping like ClickUp's signup and task-view trees — the common case where a lazy boundary is a project-local file and the user wants its static closure isolated.

#### Sub-feature B — bare-specifier form (`$lazy-entry:<bare>`)

The identifier does **not** start with `./`. Rolldown matches it against the bare specifiers used in dynamic `import()` calls in the module graph.

**Matching rule:** exact equality. No suffix, no prefix, no fuzziness.

| Identifier           | Dynamic import                 | Match? | Why                                 |
| -------------------- | ------------------------------ | ------ | ----------------------------------- |
| `react`              | `import('react')`              | ✅     | exact                               |
| `lodash-es/debounce` | `import('lodash-es/debounce')` | ✅     | exact, including subpath            |
| `debounce`           | `import('lodash-es/debounce')` | ❌     | bare is exact-only, no suffix match |
| `@scope/pkg/sub`     | `import('@scope/pkg/sub')`     | ✅     | exact                               |
| `react`              | `import('React')`              | ❌     | case-sensitive                      |

**Rationale:** bare specifiers are already the minimal canonical form — there's no boilerplate to trim. Allowing suffix would silently cross package boundaries (`debounce` would match both `rxjs/debounce` and `lodash-es/debounce`).

**Data tracking (new):** rolldown records, per module, the set of bare specifiers used to dynamically import it. Needed only for modules reached via at least one dynamic bare import.

**Primary motivation:** isolating the closure of a specific npm dependency (e.g. a heavy chart library dynamically imported on demand). Less common than path form but genuinely useful.

#### Form disambiguation

A leading `./` selects path form; anything else selects bare-specifier form. There is no silent fallback between forms:

- `./auth/signup.module.ts` is path form — if no resolved path has that suffix, the error is path-specific ("no dynamic import resolves to `…/auth/signup.module.ts`").
- `auth/signup.module.ts` is bare-specifier form — rolldown looks for a bare specifier exactly equal to `auth/signup.module.ts`; on failure the error can hint: "did you mean `./auth/signup.module.ts`?"

This syntactic discrimination means each form can be implemented, shipped, and reasoned about independently.

#### Errors (raised at config parse time, shared by both forms)

- **Zero matches**

  ```
  tag `$lazy-entry:./bar.js` did not match any dynamic import target.
  did you mean `./bar.ts`?
  ```

  When rolldown can suggest a near-match (basename collision with different extension, or the opposite form), it does.

- **Multiple matches**

  ```
  tag `$lazy-entry:./signup.module.ts` matched multiple dynamic import targets:
    - ./src/auth/signup.module.ts
    - ./src/admin/signup.module.ts
  disambiguate by narrowing, e.g. `$lazy-entry:./auth/signup.module.ts`.
  ```

No silent best-effort fallback — rolldown fails the build with an actionable message.

#### Interaction with other groups (priority handles exclusion)

Higher-priority groups claim modules first. If `initial-deps` (priority 10) captures all `$initial` modules, a lower-priority `$lazy-entry:./signup.module.ts` group receives only the modules reachable from that entry that are **not** already in `$initial`. Set-difference falls out of priority ordering — no explicit `exclude` filter is needed.

```js
codeSplitting: {
  groups: [
    { name: 'initial-deps',  tags: ['$initial'],                            priority: 10 },
    { name: 'signup-deps',   tags: ['$lazy-entry:./signup.module.ts'],      priority: 5  },
    { name: 'taskview-deps', tags: ['$lazy-entry:./task-view.module.ts'],   priority: 3  },
    { name: 'chart-deps',    tags: ['$lazy-entry:chart.js'],                priority: 2  },
  ],
}
```

#### Scope for initial implementation

- **Only dynamic `import()` targets are matched.** Named static entries from the `input` config are not covered by `$lazy-entry:*`; the `$entry:<input-key>` direction for those remains future work (see [Future directions](#future-directions)).
- **No magic comments.** Identifiers come from the specifier (bare form) or the resolved path (path form) directly — the user cannot annotate import sites.
- **No wildcards, globs, or case-insensitive match** in either form.
- **Two forms, independently shippable.** Path form and bare-specifier form can land in either order (or together). Path form is the higher-priority driver — it covers the motivating ClickUp use case and needs no new per-module data.

### Internal representation

Tag names are mapped to bit indices at build time via a registry (`$initial` → bit 0, `$lazy` → bit 1, etc.). Each module stores its tags as a `u64` bitset rather than a set of strings. Matching a module against a group's `tags` filter is a single bitwise AND + comparison — no string allocation or comparison at match time.

### Incremental build

Built-in tags like `$initial` are computed from the module graph structure (which modules are statically reachable from which entries). When a file changes during incremental/watch rebuilds, the module graph may change — an `import` statement added or removed can shift a module between `$initial` and `$lazy`. Tags must be recomputed from scratch on each rebuild during the code splitting stage. This is acceptable because tag computation piggybacks on the existing entry reachability BFS (no separate pass), and the bitset operations are negligible compared to the rest of code splitting.

### Implementation priority

**Phase 1 (shipped):**

- Built-in `$initial` tag — computed from the module graph during the generate stage
- `tags` filter in manual code splitting groups
- This alone enables initial-load parallelization: capture initial-loading modules into reasonably-sized parallel chunks

**Phase 2 (primary): `$lazy-entry:./…` path form**

- Parameterized `$lazy-entry:<./path>` tag — per-dynamic-entry reachability via resolved-path suffix
- Motivated by real-world cases (e.g. ClickUp's signup and task-view chunk grouping) where users need to isolate the static tree of a project-local lazy boundary into its own chunk
- Reuses existing resolved-path data; no new per-module metadata
- Config-time identifier resolution with hard errors on zero/ambiguous matches

**Phase 3: `$lazy-entry:<bare>` bare-specifier form**

- Parameterized `$lazy-entry:<bare>` tag — per-dynamic-entry reachability via exact bare specifier match
- Enables isolating the closure of dynamically imported npm packages
- Requires new per-module tracking: set of bare specifiers used in dynamic imports
- Can land independently of Phase 2; users writing bare identifiers before this ships see a "did you mean `./…`?" error

**Phase 4:**

- Built-in `$lazy` tag (unparameterized — all modules reachable only via `import()`)

### Future directions

These are not planned for initial implementation but could be added based on real-world needs:

- **Named-entry tags** — `$entry:<input-key>` for per-static-entry reachability. With `{ input: { main: './main.ts', admin: './admin.ts' } }`, a module reachable from the `admin` entry gets `$entry:admin`. Combined with `$initial`, this enables separating per-entry initial code across multi-entry builds:

  ```js
  groups: [{ name: 'admin-initial', tags: ['$initial', '$entry:admin'], priority: 5 }];
  ```

  The dynamic-import counterpart is covered by `$lazy-entry:<identifier>` (Phases 2–3, split by form). Magic-comment-based naming (`import(/* rolldownEntryName: 'admin' */ ...)`) is not planned — lazy identifiers come directly from the specifier or resolved path; static identifiers come from the `input` config key.

- **User-defined tags on entries** — `tags: ['browser']` in input config, propagating to reachable modules. Enables server/browser code separation in multi-entry builds.
- **Magic comments** — tag modules at import sites via magic comments (e.g., `import(/* rolldownTag: 'heavy' */ './chart.js')`). Semantics for conflicting tags at different import sites need further design (see Unresolved Questions).
- **Plugin API** — expose tags in hooks like `moduleParsed` for plugin-driven tagging.
- **Object form for `tags`** — extend `tags` to accept `{ include, exclude }` for negation:

  ```js
  { name: 'initial-deps', tags: { include: ['$initial'], exclude: ['$entry:admin'] } }
  ```

  `tags: string[]` would be shorthand for `{ include: [...] }`. Type: `string[] | { include?: string[], exclude?: string[] }`.

## Unresolved Questions

- Should tags be exposed in the plugin API (e.g., in `moduleParsed` hook)? Useful for plugins but adds API surface.
- **Conflicting tags at import sites** — if magic comments are supported in the future, what happens when two different import sites tag the same module with different names? Options include merging (union of all tags), erroring on conflicts, or last-write-wins.
- **`matchAnyTag` (OR semantics)** — `tags` uses AND semantics. Should there be a separate `matchAnyTag` option for OR semantics, or is AND sufficient for all practical use cases?

## Related

- [manual-code-splitting.md](./manual-code-splitting.md)
