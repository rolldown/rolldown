# Replace Plugin

The `replacePlugin` is a built-in Rolldown plugin that replaces the code based on string manipulation. This is an equivalent of `@rollup/plugin-replace`.

## Usage

Import and use the plugin from Rolldown's experimental exports:

```js
import { defineConfig } from 'rolldown';
import { replacePlugin } from 'rolldown/experimental';

export default defineConfig({
  input: 'src/index.js',
  output: {
    dir: 'dist',
    format: 'esm',
  },
  plugins: [
    replacePlugin(
      {
        'process.env.NODE_ENV': JSON.stringify('production'),
        __buildVersion: 15,
      },
      {
        preventAssignment: false,
      },
    ),
  ],
});
```

## Options

### `delimiters`

- **Type:** `[string, string]`
- **Default:** `["\\b", "\\b(?!\\.)"]`

Customizes how strings are matched. The default ensures word boundaries and prevents replacing property access (e.g., won't replace `process` in `process.env`).

### `preventAssignment`

- **Type:** `boolean`
- **Default:** `false`

Prevents replacing strings in variable declarations.

```js
replacePlugin(
  { 'DEBUG': 'false' },
  { preventAssignment: true },
);

// const DEBUG = true;  // Not replaced (assignment)
// console.log(DEBUG);  // Replaced with `false`
```

### `objectGuards`

- **Type:** `boolean`
- **Default:** `false`

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

### `sourcemap`

- **Type:** `boolean`
- **Default:** `false`

Generates source maps for the replacements.

## Important Notes

### Replacement Order

Keys are sorted by length (descending) to prevent partial replacements. This is crucial when you have overlapping replacement keys.

**Why order matters:**

```js
// Input code:
const apiV2 = API_URL_V2;
const api = API_URL;

replacePlugin({
  'API_URL': '"https://api.example.com"',
  'API_URL_V2': '"https://api.example.com/v2"',
});

// Without length sorting (❌ wrong):
// const apiV2 = "https://api.example.com"_V2;  // Incorrect!
// const api = "https://api.example.com";

// With length sorting (✅ correct):
// const apiV2 = "https://api.example.com/v2";  // API_URL_V2 matched first
// const api = "https://api.example.com";       // Then API_URL matched
```

The plugin automatically handles this by processing longer keys first, so you don't need to worry about the order in which you define replacements.

### Word Boundaries

By default, replacements only occur at word boundaries to prevent unintended substring replacements.

**Example:**

```js
// Input code:
const currentEnv = env;
const environment = getEnvironment();
const config = process.env.NODE_ENV;

replacePlugin({ 'env': '"production"' });

// Output:
// const currentEnv = "production";           ✅ 'env' as standalone word
// const environment = getEnvironment();      ✅ 'env' is part of 'environment'
// const config = process.env.NODE_ENV;       ✅ 'env' after '.' (property access)
```

This behavior ensures that replacing `env` doesn't accidentally break `environment` or property accesses like `process.env`. You can customize this with the `delimiters` option if needed.

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
