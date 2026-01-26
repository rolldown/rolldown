# Plugin Hook Filters

Hook filters allow Rolldown to skip unnecessary Rust-to-JS calls by evaluating filter conditions on the Rust side before invoking your plugin. This improves performance and enables better parallelization. See [Why Plugin Hook Filters](/in-depth/why-plugin-hook-filter) for more details.

## Basic Usage

Instead of checking conditions inside your hook:

```js{5}
export default function myPlugin() {
  return {
    name: 'example',
    transform(code, id) {
      if (!id.endsWith('.data')) {
        // early return
        return
      }
      // perform actual transform
      return transformedCode
    },
  }
}
```

Use the object hook format with a `filter` property:

```js{5-7}
export default function myPlugin() {
  return {
    name: 'example',
    transform: {
      filter: {
        id: /\.data$/
      },
      handler(code) {
        // perform actual transform
        return transformedCode
      },
    }
  }
}
```

Rolldown evaluates the filter on the Rust side and only calls your handler when the filter matches.

::: tip
[`@rolldown/pluginutils`](https://www.npmjs.com/package/@rolldown/pluginutils) exports some utilities for hook filters like `exactRegex` and `prefixRegex`.
:::

## Filter Properties

In addition to `id`, you can also filter based on `moduleType` and the module's source code. The `filter` property works similarly to [`createFilter` from `@rollup/pluginutils`](https://github.com/rollup/plugins/blob/master/packages/pluginutils/README.md#createfilter).

- If multiple values are passed to `include`, the filter matches if **any** of them match.
- If a filter has both `include` and `exclude`, `exclude` takes precedence.
- If multiple filter properties are specified, the filter matches when all of the specified properties match. In other words, if even one property fails to match, it is excluded, regardless of the other properties. For example, the following filter matches a module only if its file names ends with `.js`, its source code contains `foo`, and does not contain `bar`:
  ```js
  {
    id: {
      include: /\.js$/,
      exclude: /\.ts$/
    },
    code: {
      include: 'foo',
      exclude: 'bar'
    }
  }
  ```

The following properties are supported by each hook:

- `resolveId` hook: `id`
- `load` hook: `id`
- `transform` hook: `id`, `moduleType`, `code`

See [`HookFilter`](/reference/Interface.HookFilter) as well.

> [!NOTE]
> `id` is treated as a glob pattern when you pass a `string`, and treated as a regular expression when you pass a `RegExp`.
> In the `resolve` hook, `id` must be a `RegExp`. `string`s are not allowed.
> This is because the `id` value in `resolveId` is the exact text written in the import statement and usually not an absolute path, while glob patterns are designed to match absolute paths.

## Composable Filters

For more complex filtering logic, Rolldown provides composable filter expressions via the [`@rolldown/pluginutils`](https://github.com/rolldown/rolldown/tree/main/packages/pluginutils) package. These allow you to build filters using logical operators like `and`, `or`, and `not`.

```js
import { and, id, include, moduleType } from '@rolldown/pluginutils';

export default function myPlugin() {
  return {
    name: 'my-plugin',
    transform: {
      filter: [include(and(id(/\.ts$/), moduleType('ts')))],
      handler(code, id) {
        // Only called for .ts files with moduleType 'ts'
        return transformedCode;
      },
    },
  };
}
```

See the [`@rolldown/pluginutils` README](https://github.com/rolldown/rolldown/tree/main/packages/pluginutils#readme) for the full API reference.

## Interoperability

Plugin hook filters are supported in Rollup 4.38.0+, Vite 6.3.0+, and all versions of Rolldown.

### Supporting Older Versions

If you're authoring a plugin that needs to support older versions of Rollup (< 4.38.0) or Vite (< 6.3.0), you can provide a fallback implementation that works in both environments.

The strategy is to use the object hook format with filters when available, and fall back to a regular function that checks conditions internally for older versions:

```js
const idFilter = /\.data$/;

export default function myPlugin() {
  return {
    name: 'my-plugin',
    transform: {
      // Filter is used by Rolldown and newer Rollup/Vite versions
      filter: { id: idFilter },
      // Handler is called when filter matches
      handler(code, id) {
        // Double-check in handler for compatibility with older versions
        // This is only necessary if you're supporting older versions
        if (!idFilter.test(id)) {
          return null;
        }
        // perform actual transform
        return transformedCode;
      },
    },
  };
}
```

This approach ensures your plugin will:

- Use filters for optimal performance in Rolldown and newer Rollup/Vite versions
- Still work correctly in older versions (they will call the handler for all files, but the internal check ensures correct behavior)

> [!TIP]
> When supporting older versions, keep both the filter pattern and the internal check in sync to avoid confusion.

### `moduleType` Filter

The [Module Type concept](/in-depth/module-types) does not exist in Rollup / Vite 7 and below. For that reason, the `moduleType` filter is not supported by those tools and will be ignored.

### Composable Filters

Composable filters are currently only supported in Rolldown. They are not yet supported in Vite, Rolldown-Vite, or unplugin.
