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

A hook of a parallel plugin (`defineParallelPlugin`) is never cached, also with `cache: true`.

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
