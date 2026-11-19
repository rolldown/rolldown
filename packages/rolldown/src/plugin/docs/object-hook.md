## Additional Properties

### order

- Type: `"pre" | "post" | null`

If there are several plugins implementing this hook, either run this plugin first (`"pre"`), last (`"post"`), or in the user-specified position (no value or `null`).

If several plugins use `"pre"` or `"post"`, Rolldown runs them in the user-specified order. This option can be used for all plugin hooks.

#### Example

```js
export default function resolveFirst() {
  return {
    name: 'resolve-first',
    resolveId: {
      order: 'pre',
      handler(source) {
        if (source === 'external') {
          return { id: source, external: true };
        }
        return null;
      },
    },
  };
}
```

### filter

- Type: [`HookFilter`](/reference/Interface.HookFilter) | `TopLevelFilterExpression`[] (depends on hook)

Run this plugin hook only when the specified filter returns true. This property is only available for [`resolveId`](/reference/Interface.Plugin#resolveid), [`load`](/reference/Interface.Plugin#load), [`transform`](/reference/Interface.Plugin#transform), and [`renderChunk`](/reference/Interface.Plugin#renderchunk) hooks. See [Plugin Hook Filters](/apis/plugin-api/hook-filters) for the properties supported by each hook.

#### Example

```js
export default function jsxAdditionalTransform() {
  return {
    name: 'jsxAdditionalTransform',
    transform: {
      filter: {
        id: '*.jsx',
        code: '<Custom',
      },
      handler(code) {
        // transform <Custom /> here
      },
    },
  };
}
```

### cache

- Type: `boolean`

`true` lets Rolldown skip this hook and reuse the result that it saved in an earlier build. This property is only available for [`resolveId`](/reference/Interface.Plugin#resolveid), [`resolveDynamicImport`](/reference/Interface.Plugin#resolvedynamicimport), [`load`](/reference/Interface.Plugin#load), and [`transform`](/reference/Interface.Plugin#transform) hooks. It has an effect only when the `experimental.cache` option is set.

The cache is off by default: without `cache: true`, the hook runs in every build. The `experimental.cache.assumeCacheableHooks` option can treat a hook without `cache` as if it had `cache: true`, but it never changes a hook with `cache: false`.

A module gets a saved result only when every one of these hooks that ran for it has `cache: true`. A hook that a `filter` skips for the module does not count.

By setting `cache: true`, the plugin promises that skipping the hook loses nothing that another hook needs. For example, a `transform` hook that stores data in a variable for a later `renderChunk` hook must not set `cache: true`, because the variable is empty when the `transform` hook is skipped.

Such a hook can return the data as `meta` instead. Rolldown saves the `meta` and the `moduleSideEffects` that a hook returns, and gives them back when it skips the hook. A later hook reads them with `this.getModuleInfo(id)`, as in a build without the cache.

In a hook with `cache: true`, do not read or change `meta` or `moduleSideEffects` through `this.getModuleInfo(id)`, for any `id`. For example, do not write `this.getModuleInfo(id).meta.icons = ['search']`. Such a use, and a call of `this.load`, gives the module of the hook call no saved result.

Rolldown saves `meta` as JSON. A value in it must be a string, a finite number, a boolean, `null`, an array or a plain object. An object must not be used two times in one `meta`. `undefined` is allowed only as the value of a top-level key of `meta`. Inside a value, a getter, a setter, or a frozen or sealed object cannot be saved either. When a hook returns another value, such as a `Set` or a function, Rolldown saves no result for that module and prints the warning `PERSISTENT_CACHE_UNSAVABLE_META`. Each module gets its own copy of the saved values, so an object that a hook returns for several modules is not shared between them after a restart. Rolldown saves the logs of a hook as JSON in the same way. A log that changes when it is saved as JSON, such as a log with an `Error` in it, gives the module no saved result.

#### Example

```js
export default function replaceVersion() {
  return {
    name: 'replace-version',
    transform: {
      cache: true,
      filter: { id: /\.js$/, code: '__VERSION__' },
      handler(code) {
        // The result depends only on `code`, so the saved result is always correct.
        return code.replaceAll('__VERSION__', '"1.0.0"');
      },
    },
  };
}
```

### ~~sequential~~

- Type: `boolean`

#### Deprecated

This option is only for Rollup plugin compatibility. Hooks always work as `sequential: true`.
