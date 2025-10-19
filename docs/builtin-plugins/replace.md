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

_To be documented_

## Limitations

_To be documented_
