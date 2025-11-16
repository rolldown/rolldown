# Non ESM Output Formats

Rolldown supports non-ESM output formats. Some features in ESM are not supported in non-ESM formats and Rolldown will emit messages or provide polyfills for them.

## Top Level Await

Top level await is not supported in non-ESM formats. Rolldown outputs an error if it encounters top level await when the output format is not ESM.

## `import.meta`

`import.meta` is a syntax error in non-ESM formats. To avoid that from happening, Rolldown replaces `import.meta` with other values.

### Well-known `import.meta` properties

Rolldown supports the following well-known `import.meta` properties:

- `import.meta.url`
- `import.meta.dirname`
- `import.meta.filename`

These properties are polyfilled when the output format is CJS. In other formats, it will be handled as same as the other properties.

:::: tip Polyfilling `import.meta.url` in IIFE and UMD

Rollup supports polyfilling `import.meta.url` in IIFE and UMD formats. However, Rolldown does not support this feature. If you need to polyfill it, you can use the following config:

::: code-group

```ts [rolldown.config.ts (IIFE)]
import { defineConfig } from 'rolldown';

const importMetaUrlPolyfillVariableName = '__import_meta_url__';

export default defineConfig({
  transform: {
    define: {
      'import.meta.url': importMetaUrlPolyfillVariableName,
    },
  },
  output: {
    format: 'iife',
    intro:
      "var _documentCurrentScript = typeof document !== 'undefined' ? document.currentScript : null;" +
      `var ${importMetaUrlPolyfillVariableName} = (_documentCurrentScript && _documentCurrentScript.tagName.toUpperCase() === 'SCRIPT' && _documentCurrentScript.src || new URL('main.js', document.baseURI).href)`,
  },
});
```

```ts [rolldown.config.ts (UMD)]
import { defineConfig } from 'rolldown';

const importMetaUrlPolyfillVariableName = '__import_meta_url__';

export default defineConfig({
  transform: {
    define: {
      'import.meta.url': importMetaUrlPolyfillVariableName,
    },
  },
  output: {
    format: 'umd',
    intro:
      "var _documentCurrentScript = typeof document !== 'undefined' ? document.currentScript : null;" +
      `var ${importMetaUrlPolyfillVariableName} = (typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.tagName.toUpperCase() === 'SCRIPT' && _documentCurrentScript.src || new URL('main.js', document.baseURI).href))`,
  },
});
```

:::

::::

### Other properties and `import.meta` object itself

Other properties and `import.meta` object itself are replaced with `{}`. Since this does not keep the original value, Rolldown emits a warning in this case.
