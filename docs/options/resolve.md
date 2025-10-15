# Resolve Options

- **Type:** `object`
- **Default:** See individual options

Configuration for module resolution.

## alias

- **Type:** `Record<string, string[] | string | false>`
- **Optional:** Yes ✅
- **Path:** `resolve.alias`

Map of module aliases.

### Examples

```js
export default {
  resolve: {
    alias: {
      '@': '/src',
      'utils': './src/utils',
    },
  },
};
```

### In-depth

:::warning
`resolve.alias` will not call `resolveId` hooks of other plugins. If you want to call `resolveId` hooks of other plugins, use `aliasPlugin` from `rolldown/experimental` instead. See [this issue](https://github.com/rolldown/rolldown/issues/3615) for more discussion.
:::

## aliasFields

- **Type:** `string[][]`
- **Optional:** Yes ✅
- **Path:** `resolve.aliasFields`

Fields in package.json to check for aliased paths.

## conditionNames

- **Type:** `string[]`
- **Default:** Platform-dependent
- **Path:** `resolve.conditionNames`

Condition names to use when resolving exports in package.json. Defaults based on platform and import kind:

- **Browser platform**: `["import", "browser", "default"]` for import statements, `["require", "browser", "default"]` for require() calls
- **Node platform**: `["import", "node", "default"]` for import statements, `["require", "node", "default"]` for require() calls
- **Neutral platform**: `["import", "default"]` for import statements, `["require", "default"]` for require() calls

## extensionAlias

- **Type:** `Record<string, string[]>`
- **Optional:** Yes ✅
- **Path:** `resolve.extensionAlias`

Map of extensions to alternative extensions. Useful for resolving TypeScript files when importing with `.js` extension. With this configuration, `import './foo.js'` will try to resolve to `foo.ts` first, then fall back to `foo.js`.

### Examples

```js
export default {
  resolve: {
    extensionAlias: {
      '.js': ['.ts', '.js'],
    },
  },
};
```

## exportsFields

- **Type:** `string[][]`
- **Optional:** Yes ✅
- **Path:** `resolve.exportsFields`

Fields in package.json to check for exports.

## extensions

- **Type:** `string[]`
- **Default:** `['.tsx', '.ts', '.jsx', '.js', '.json']`
- **Path:** `resolve.extensions`

Extensions to try when resolving files. These are tried in order from first to last.

## mainFields

- **Type:** `string[]`
- **Default:** Platform-dependent
- **Path:** `resolve.mainFields`

Fields in package.json to check for entry points. Defaults based on platform:

- **Node**: `['main', 'module']`
- **Browser**: `['browser', 'module', 'main']`
- **Neutral**: `[]` (relies on exports field)

## mainFiles

- **Type:** `string[]`
- **Default:** `['index']`
- **Path:** `resolve.mainFiles`

Filenames to try when resolving directories.

## modules

- **Type:** `string[]`
- **Default:** `['node_modules']`
- **Path:** `resolve.modules`

Directories to search for modules.

## symlinks

- **Type:** `boolean`
- **Default:** `true`
- **Path:** `resolve.symlinks`

Whether to follow symlinks when resolving modules.
