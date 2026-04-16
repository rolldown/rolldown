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

`$lazy-entry:<identifier>` is a parameterized tag — the identifier after the colon selects a specific dynamic import target in the module graph. Rolldown collects every `$lazy-entry:*` reference from `codeSplitting.groups` at config parse time, resolves each identifier to exactly one dynamic import target, and tags that target plus its static-import closure.

#### Matching rule

A dynamic-import target module matches `<identifier>` if _either_:

1. A **bare specifier** that dynamically imports this module **equals** the identifier exactly, or
2. The module's **resolved path** has a path-segment-aligned suffix matching the identifier.

Extensions are included on both sides — no stripping, no case-insensitive matching. Segments align on `/` boundaries (no mid-segment matching).

| Identifier | Source import | Resolved path | Matches? | Why |
|---|---|---|---|---|
| `react` | `import('react')` | `…/node_modules/react/index.js` | ✅ | bare specifier, exact |
| `react/index.js` | `import('react')` | `…/node_modules/react/index.js` | ✅ | resolved-path suffix |
| `debounce` | `import('lodash-es/debounce')` | — | ❌ | bare is exact-only |
| `lodash-es/debounce` | `import('lodash-es/debounce')` | — | ✅ | bare specifier, exact |
| `signup.module.ts` | — | `/project/src/auth/signup.module.ts` | ✅ | resolved-path suffix |
| `auth/signup.module.ts` | — | `/project/src/auth/signup.module.ts` | ✅ | resolved-path suffix |
| `./signup.module.ts` | — | — | ❌ | not a bare specifier; leading `./` not a valid path-segment suffix |
| `bar.ts` | `import('./bar.js')` | `/project/src/bar.ts` | ✅ | resolved-path suffix (resolution picked the `.ts` source) |
| `bar.js` | `import('./bar.js')` | `/project/src/bar.ts` | ❌ | resolved extension is `.ts`, not `.js` |
| `foo.js` | — | `/project/src/foo.ts` | ❌ | different extension |
| `th/signup.module.ts` | — | `/project/src/auth/signup.module.ts` | ❌ | mid-segment — `th` is not a full path segment |

#### Why the asymmetry (bare = exact, path = suffix)

Bare specifiers are already the minimal canonical form — `react`, `lodash-es/debounce`. There is no boilerplate to trim; writing the whole thing costs nothing and stays unambiguous across packages (`debounce` must not silently match both `rxjs/debounce` and `lodash-es/debounce`).

Resolved paths carry absolute-path prefixes (`/Users/you/project/…`) that users don't care about. Path-suffix match strips that noise cleanly. Segment alignment prevents silent substring matches such as `signup.module.ts` accidentally matching `foosignup.module.ts`.

#### Errors (raised at config parse time)

- **Zero matches** — no dynamic import target resolves the identifier:

  ```
  tag `$lazy-entry:bar.js` did not match any dynamic import target.
  did you mean `bar.ts`?
  ```

  When rolldown can suggest a near-match (e.g. identical basename with a different extension), it does.

- **Multiple matches** — the identifier resolves to two or more modules:

  ```
  tag `$lazy-entry:signup.module.ts` matched multiple dynamic import targets:
    - ./src/auth/signup.module.ts
    - ./src/admin/signup.module.ts
  disambiguate by narrowing, e.g. `$lazy-entry:auth/signup.module.ts`.
  ```

No silent best-effort fallback — rolldown fails the build with an actionable message.

#### Interaction with other groups (priority handles exclusion)

Higher-priority groups claim modules first. If `initial-deps` (priority 10) captures all `$initial` modules, a lower-priority `$lazy-entry:signup` group receives only the modules reachable from `signup` that are **not** already in `$initial`. Set-difference falls out of priority ordering — no explicit `exclude` filter is needed.

```js
codeSplitting: {
  groups: [
    { name: 'initial-deps',  tags: ['$initial'],                       priority: 10 },
    { name: 'signup-deps',   tags: ['$lazy-entry:signup.module.ts'],   priority: 5  },
    { name: 'taskview-deps', tags: ['$lazy-entry:task-view.module.ts'], priority: 3  },
  ],
}
```

#### Scope for initial implementation

- **Only dynamic `import()` targets are matched.** Named static entries from the `input` config are not covered by `$lazy-entry:*`; the `$entry:<input-key>` direction for those remains future work (see [Future directions](#future-directions)).
- **No magic comments.** Identifiers come from the import specifier or the resolved path directly — the user cannot annotate import sites.
- **No wildcards, globs, or case-insensitive match.** Exact bare-specifier match or extension-strict path-suffix match only.
- **Per-module tracking:** rolldown records the resolved path (already tracked) plus the set of bare specifiers used in dynamic imports of each module (new — needed only for modules reached via at least one dynamic bare import).

### Internal representation

Tag names are mapped to bit indices at build time via a registry (`$initial` → bit 0, `$lazy` → bit 1, etc.). Each module stores its tags as a `u64` bitset rather than a set of strings. Matching a module against a group's `tags` filter is a single bitwise AND + comparison — no string allocation or comparison at match time.

### Incremental build

Built-in tags like `$initial` are computed from the module graph structure (which modules are statically reachable from which entries). When a file changes during incremental/watch rebuilds, the module graph may change — an `import` statement added or removed can shift a module between `$initial` and `$lazy`. Tags must be recomputed from scratch on each rebuild during the code splitting stage. This is acceptable because tag computation piggybacks on the existing entry reachability BFS (no separate pass), and the bitset operations are negligible compared to the rest of code splitting.

### Implementation priority

**Phase 1 (shipped):**

- Built-in `$initial` tag — computed from the module graph during the generate stage
- `tags` filter in manual code splitting groups
- This alone enables initial-load parallelization: capture initial-loading modules into reasonably-sized parallel chunks

**Phase 2:**

- Parameterized `$lazy-entry:<identifier>` tag — per-dynamic-entry reachability
- Motivated by real-world cases (e.g. ClickUp's signup and task-view chunk grouping) where users need to isolate the static tree of a specific lazy boundary into its own chunk, independent of what other route-specific trees contain
- Config-time identifier resolution with hard errors on zero/ambiguous matches — no silent fallbacks

**Phase 3:**

- Built-in `$lazy` tag (unparameterized — all modules reachable only via `import()`)

### Future directions

These are not planned for initial implementation but could be added based on real-world needs:

- **Named-entry tags** — `$entry:<input-key>` for per-static-entry reachability. With `{ input: { main: './main.ts', admin: './admin.ts' } }`, a module reachable from the `admin` entry gets `$entry:admin`. Combined with `$initial`, this enables separating per-entry initial code across multi-entry builds:

  ```js
  groups: [
    { name: 'admin-initial', tags: ['$initial', '$entry:admin'], priority: 5 },
  ];
  ```

  The dynamic-import counterpart is covered by `$lazy-entry:<identifier>` (Phase 2). Magic-comment-based naming (`import(/* rolldownEntryName: 'admin' */ ...)`) is not planned — lazy identifiers come directly from the specifier or resolved path; static identifiers come from the `input` config key.

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
