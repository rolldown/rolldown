# Maintenance Guide

A plugin for `vite` that handles the transformation of JSON files into JavaScript modules, ported from `Vite`'s [jsonPlugin](https://github.com/vitejs/rolldown-vite/blob/fa33494/packages/vite/src/node/plugins/json.ts).

> [!NOTE]
> This plugin is exclusive to `vite`; external use is not recommended.
> Its API may change between minor versions of `rolldown`, but
> stays compatible within the same minor version.

## 📦 What it does

This plugin processes JSON files and transforms them into JavaScript modules. The resulting JavaScript either directly exports the parsed JSON or provides a wrapper around `JSON.parse()` depending on the configuration.

## 🚀 Debug Usage

```js
import { defineConfig } from 'rolldown';
import { viteJsonPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './main.ts',
  },
  plugins: [
    viteJsonPlugin({
      minify: false,
      namedExports: false,
      stringify: 'auto',
    }),
  ],
});
```

## ⚙️ Options

| Option         | Type                  | Description                                                                        | Default  |
| -------------- | --------------------- | ---------------------------------------------------------------------------------- | -------- |
| `minify`       | `boolean`             | Whether to minify the JSON content (remove spaces and formatting) for `stringify`. | `false`  |
| `namedExports` | `boolean`             | Whether to use named exports for JSON properties (useful for ESM compatibility).   | `false`  |
| `stringify`    | `JsonPluginStringify` | Determines when the JSON content should be stringified into a `JSON.parse()` call. | `"auto"` |

### 🧩 `JsonPluginStringify`

This option controls how the JSON content is processed:

- **`"auto"`**: Automatically decides whether to stringify the JSON based on file size.
- **`true`**: Always stringify the content, even for small JSON files.
- **`false`**: Never stringify, just export the JSON as is.
