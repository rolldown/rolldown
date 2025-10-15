# Sourcemap Options

- **Type:** `boolean | 'inline' | 'hidden' | SourcemapOptions`
- **Default:** `false`

Control source map generation and configuration.

## sourcemap

- **Type:** `boolean | 'inline' | 'hidden'`
- **Default:** `false`
- **Path:** `output.sourcemap`

Controls source map generation.

### Examples

```js
export default {
  output: {
    sourcemap: true,
  },
};
```

### In-depth

The available options:

- `false`: No source maps
- `true`: Generates separate `.map` files
- `'inline'`: Embeds source map as data URI in output
- `'hidden'`: Generates source maps without adding `//# sourceMappingURL` comment

## sourcemapBaseUrl

- **Type:** `string`
- **Optional:** Yes ✅
- **Path:** `output.sourcemapBaseUrl`

Base URL to prepend to source paths in the source map. This is useful when deploying source maps to a different location than your code, such as a CDN or separate debugging server.

### Examples

```js
export default {
  output: {
    sourcemap: true,
    sourcemapBaseUrl: 'https://example.com/src/',
  },
};
```

## sourcemapDebugIds

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `output.sourcemapDebugIds`

Add debug IDs to source maps for better error tracking and debugging. When enabled, Rolldown adds unique identifiers to source maps, which can be used by error tracking services to match stack traces to specific builds.

### Examples

```js
export default {
  output: {
    sourcemap: true,
    sourcemapDebugIds: true,
  },
};
```

## sourcemapIgnoreList

- **Type:** `boolean | string | RegExp | ((source: string, sourcemapPath: string) => boolean)`
- **Default:** `/node_modules/`
- **Path:** `output.sourcemapIgnoreList`

Control which source files are included in the sourcemap ignore list. Files in the ignore list are excluded from debugger stepping and error stack traces.

### Examples

```js
// Use RegExp for better performance
export default {
  output: {
    sourcemap: true,
    sourcemapIgnoreList: /node_modules/,
  },
};
```

```js
// Use string pattern
export default {
  output: {
    sourcemap: true,
    sourcemapIgnoreList: 'vendor',
  },
};
```

```js
// Use function (has performance overhead)
export default {
  output: {
    sourcemap: true,
    sourcemapIgnoreList: (source, sourcemapPath) => {
      return source.includes('node_modules') || source.includes('.min.');
    },
  },
};
```

### In-depth

The available options:

- `false`: Include no source files in the ignore list (do not ignore any files)
- `true`: Use the default ignore list (e.g., ignore node_modules and minified files)
- `string`: Files containing this string in their path will be included in the ignore list
- `RegExp`: Files matching this regular expression will be included in the ignore list
- `function`: Custom function to determine if a source should be ignored

:::tip Performance
Using static values (`boolean`, `string`, or `RegExp`) is significantly more performant than functions. Calling JavaScript functions from Rust has extremely high overhead, so prefer static patterns when possible.
:::

## sourcemapPathTransform

- **Type:** `(relativeSourcePath: string, sourcemapPath: string) => string`
- **Optional:** Yes ✅
- **Path:** `output.sourcemapPathTransform`

Function to transform source paths in the source map.

### Examples

```js
export default {
  output: {
    sourcemap: true,
    sourcemapPathTransform: (relativeSourcePath, sourcemapPath) => {
      // Remove 'src/' prefix from all source paths
      return relativeSourcePath.replace(/^src\//, '');
    },
  },
};
```

```js
import path from 'node:path';

export default {
  output: {
    sourcemap: true,
    sourcemapPathTransform: (relativeSourcePath, sourcemapPath) => {
      return path.relative(process.cwd(), relativeSourcePath);
    },
  },
};
```

### In-depth

This allows you to customize how source file paths appear in the generated source map, which is useful for:

- Removing or modifying path prefixes
- Normalizing paths across different environments
- Adjusting paths for deployment scenarios
