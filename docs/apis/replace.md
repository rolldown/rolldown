# Replace Plugin

The `replacePlugin` is a built-in Rolldown plugin that replaces targeted strings in files during bundling. It's commonly used for injecting environment variables, build constants, and feature flags into your code.

## Why This Is Needed

When bundling JavaScript applications, you often need to replace placeholder strings with actual values at build time. Common scenarios include:

- Converting `process.env.NODE_ENV` to `"production"` to enable optimizations and dead code elimination
- Injecting API endpoints that differ between development and production
- Embedding build metadata like version numbers and timestamps
- Toggling feature flags without runtime overhead

By performing these replacements during bundling, the final code contains literal values that can be optimized by minifiers and don't require runtime lookups.

## Usage

```js
import { defineConfig } from 'rolldown';
import { replacePlugin } from 'rolldown/plugins';

export default defineConfig({
  input: 'src/index.js',
  output: {
    dir: 'dist',
    format: 'esm',
  },
  plugins: [
    replacePlugin({
      'process.env.NODE_ENV': JSON.stringify('production'),
      '__DEV__': 'false',
      '__VERSION__': JSON.stringify('1.0.0'),
    }),
  ],
});
```

## Options

### Basic Syntax

```js
replacePlugin(values, options);
```

- **`values`**: An object mapping strings to their replacements
- **`options`**: Optional configuration for fine-tuning behavior

### `values`

Type: `Record<string, string | number>`

An object where keys are the strings to search for and values are their replacements.

```js
replacePlugin({
  'process.env.NODE_ENV': JSON.stringify('production'),
  '__DEV__': 'false',
  '__API_URL__': JSON.stringify('https://api.example.com'),
});
```

**Important**: Use `JSON.stringify()` for string values to ensure they're properly quoted in the output.

### `options`

#### `delimiters`

Type: `[string, string]`
Default: `["\\b", "\\b(?!\\.)"]`

Customizes how strings are matched. The default ensures word boundaries and prevents replacing property access (e.g., won't replace `process` in `process.env`).

#### `preventAssignment`

Type: `boolean`
Default: `false`

Prevents replacing strings in variable declarations.

```js
replacePlugin(
  { 'DEBUG': 'false' },
  { preventAssignment: true },
);

// const DEBUG = true;  // Not replaced
// console.log(DEBUG);  // Replaced with 'false'
```

#### `objectGuards`

Type: `boolean`
Default: `false`

Automatically replaces `typeof` checks for object paths.

```js
replacePlugin(
  { 'process.env.NODE_ENV': JSON.stringify('production') },
  { objectGuards: true },
);

// Also replaces:
// typeof process → "object"
// typeof process.env → "object"
```

#### `sourcemap`

Type: `boolean`
Default: `false`

Generates source maps for the replacements.

## Examples

### Basic Usage

```js
replacePlugin({
  'process.env.NODE_ENV': JSON.stringify('production'),
  '__DEV__': 'false',
  '__VERSION__': JSON.stringify('1.2.3'),
});

// Code before:
if (process.env.NODE_ENV === 'development') {
  console.log('Dev mode');
}

// Code after:
if ('production' === 'development') {
  console.log('Dev mode');
}
```

### With Options

```js
replacePlugin(
  {
    'process.env.NODE_ENV': JSON.stringify('production'),
    'DEBUG': 'false',
  },
  {
    objectGuards: true, // Also replaces: typeof process → "object"
    preventAssignment: true, // Won't replace: const DEBUG = true
  },
);
```

## Important Notes

### Replacement Order

Keys are sorted by length (descending) to prevent partial replacements:

```js
replacePlugin({
  'process.env': JSON.stringify({ NODE_ENV: 'production' }),
  'process': JSON.stringify({ env: {} }),
});
// 'process.env' is matched before 'process'
```

### Word Boundaries

By default, replacements only occur at word boundaries:

```js
replacePlugin({ 'env': '"production"' });

// 'env' → '"production"' ✅
// 'environment' → unchanged ✅ (no word boundary)
// 'process.env' → unchanged ✅ (preceded by '.')
```

### Type Conversion

Non-string values are auto-converted with a warning:

```js
replacePlugin({ '__buildVersion': 15 }); // Converts to "15"
// Warning: Some values provided to `replacePlugin` are not strings...
```

## Migration from @rollup/plugin-replace

### Feature Comparison

| Feature         | @rollup/plugin-replace       | rolldown                        |
| --------------- | ---------------------------- | ------------------------------- |
| API             | `replace({ values: {...} })` | `replacePlugin({...}, options)` |
| Function values | ✅ `() => value`             | ❌ Static values only           |
| File filtering  | ✅ include/exclude           | ❌ All files                    |
| Type conversion | Manual                       | Auto (with warning)             |
| Performance     | JavaScript                   | Rust (faster)                   |

### Migration Example

```js
// Before (@rollup/plugin-replace)
replace({
  values: { '__VERSION__': () => getVersion() },
  include: ['src/**/*.js'],
});

// After (rolldown)
replacePlugin({
  '__VERSION__': JSON.stringify(getVersion()),
});
```
