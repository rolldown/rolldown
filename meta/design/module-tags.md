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

### Implementation priority

**Phase 1:**

- Built-in `$initial` tag — computed from the module graph during the generate stage
- `tags` filter in manual code splitting groups
- This alone enables initial-load parallelization: capture initial-loading modules into reasonably-sized parallel chunks

**Phase 2:**

- Built-in `$lazy` tag

### Future directions

These are not planned for initial implementation but could be added based on real-world needs:

- **Per-entry tags** — `$entry:<name>` to identify which entry a module is reachable from. With `{ input: { main: './main.ts', admin: './admin.ts' } }`, a module reachable from both gets `$entry:main` and `$entry:admin`. Combined with `$initial`, this enables per-entry grouping:

  ```js
  groups: [
    // Initial deps shared by both entries
    { name: 'shared-initial', tags: ['$initial', '$entry:main', '$entry:admin'] },
    // Initial deps only for admin
    { name: 'admin-initial', tags: ['$initial', '$entry:admin'] },
  ];
  ```

  This also works for dynamic entries by naming them via magic comments:

  ```js
  import(/* rolldownEntryName: 'admin' */ './admin-panel.js');
  // rolldown automatically tags reachable modules with $entry:admin
  ```

  ```js
  { name: 'admin-deps', tags: ['$entry:admin'] }
  ```

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
