# Advanced Chunks Options

- **Type:** `object`
- **Optional:** Yes ✅

Allows manual chunking with fine-grained control, similar to webpack's `optimization.splitChunks`.

## Examples

### Basic vendor chunk

```js
export default {
  output: {
    advancedChunks: {
      minSize: 20000,
      groups: [
        {
          name: 'vendor',
          test: /node_modules/,
          priority: 10,
        },
      ],
    },
  },
};
```

### Multiple chunk groups with priorities

```js
export default {
  output: {
    advancedChunks: {
      groups: [
        {
          name: 'react-vendor',
          test: /node_modules[\\/]react/,
          priority: 20,
        },
        {
          name: 'ui-vendor',
          test: /node_modules[\\/](antd|@mui)/,
          priority: 15,
        },
        {
          name: 'vendor',
          test: /node_modules/,
          priority: 10,
        },
        {
          name: 'common',
          minShareCount: 2,
          minSize: 10000,
          priority: 5,
        },
      ],
    },
  },
};
```

### Size-based splitting

```js
export default {
  output: {
    advancedChunks: {
      groups: [
        {
          name: 'large-libs',
          test: /node_modules/,
          minSize: 100000, // 100KB
          maxSize: 250000, // 250KB
          priority: 10,
        },
      ],
    },
  },
};
```

## includeDependenciesRecursively

- **Type:** `boolean`
- **Default:** `true`
- **Path:** `output.advancedChunks.includeDependenciesRecursively`

By default, each group will also include captured modules' dependencies. This reduces the chance of generating circular chunks.

### In-depth

If you want to disable this behavior, it's recommended to both set:

- `preserveEntrySignatures: false | 'allow-extension'`
- `strictExecutionOrder: true`

to avoid generating invalid chunks.

## minSize

- **Type:** `number`
- **Optional:** Yes ✅
- **Path:** `output.advancedChunks.minSize`

Global fallback for group `minSize` if not specified in the group.

## maxSize

- **Type:** `number`
- **Optional:** Yes ✅
- **Path:** `output.advancedChunks.maxSize`

Global fallback for group `maxSize` if not specified in the group.

## maxModuleSize

- **Type:** `number`
- **Optional:** Yes ✅
- **Path:** `output.advancedChunks.maxModuleSize`

Global fallback for group `maxModuleSize` if not specified in the group.

## minModuleSize

- **Type:** `number`
- **Optional:** Yes ✅
- **Path:** `output.advancedChunks.minModuleSize`

Global fallback for group `minModuleSize` if not specified in the group.

## minShareCount

- **Type:** `number`
- **Optional:** Yes ✅
- **Path:** `output.advancedChunks.minShareCount`

Global fallback for group `minShareCount` if not specified in the group.

## groups

- **Type:** `Array<GroupConfig>`
- **Optional:** Yes ✅
- **Path:** `output.advancedChunks.groups`

Groups to be used for advanced chunking.

### name

- **Type:** `string | ((moduleId: string, ctx: ChunkingContext) => string | NullValue)`
- **Path:** `output.advancedChunks.groups[].name`

Name of the group. Used as the chunk name and replaces the `[name]` placeholder in `chunkFileNames`.

#### Examples

Static name:

```js
{
  name: 'libs',
  test: /node_modules/
}
```

Dynamic name:

```js
{
  name: (moduleId) => {
    if (moduleId.includes('node_modules')) {
      return 'libs';
    }
    return 'app';
  },
  minSize: 100 * 1024
}
```

:::warning
Constraints like `minSize`, `maxSize`, etc. are applied separately for different names returned by the function.
:::

### test

- **Type:** `string | RegExp | ((id: string) => boolean | undefined | void)`
- **Optional:** Yes ✅
- **Path:** `output.advancedChunks.groups[].test`

Controls which modules are captured in this group.

#### In-depth

The available options:

- String: Module IDs containing this string will be captured
- RegExp: Module IDs matching this pattern will be captured
- Function: Modules for which `test(id)` returns `true` will be captured
- Empty: All modules are considered matched

:::warning
When using regular expressions, use `[\\/]` to match path separators instead of `/` to avoid issues on Windows.

✅ Recommended: `/node_modules[\\/]react/`

❌ Not recommended: `/node_modules/react/`
:::

### priority

- **Type:** `number`
- **Default:** `0`
- **Path:** `output.advancedChunks.groups[].priority`

Priority of the group. Groups with higher priority are chosen first.

#### Examples

```js
{
  advancedChunks: {
    groups: [
      {
        name: 'react',
        test: /node_modules[\\/]react/,
        priority: 1,
      },
      {
        name: 'other-libs',
        test: /node_modules/,
        priority: 2,
      },
    ];
  }
}
```

### minSize

- **Type:** `number`
- **Default:** `0`
- **Path:** `output.advancedChunks.groups[].minSize`

Minimum size in bytes of the desired chunk. If the accumulated size is smaller, the group is ignored.

### minShareCount

- **Type:** `number`
- **Default:** `1`
- **Path:** `output.advancedChunks.groups[].minShareCount`

Controls if a module should be captured based on how many entry chunks reference it.

### maxSize

- **Type:** `number`
- **Default:** `Infinity`
- **Path:** `output.advancedChunks.groups[].maxSize`

If the accumulated size exceeds this value, the group will be split into multiple groups.

### maxModuleSize

- **Type:** `number`
- **Default:** `Infinity`
- **Path:** `output.advancedChunks.groups[].maxModuleSize`

A module can only be captured if its size is smaller or equal to this value.

### minModuleSize

- **Type:** `number`
- **Default:** `0`
- **Path:** `output.advancedChunks.groups[].minModuleSize`

A module can only be captured if its size is larger or equal to this value.

## See Also

- [In-depth: Advanced Chunks](../in-depth/advanced-chunks.md)
