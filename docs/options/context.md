# context

- **Type:** `string`
- **Optional:** Yes ✅

The value of `this` at the top level of each output chunk. For IIFE and UMD formats, this defaults to `'window'` or `'global'` depending on the platform.

## Examples

### Set custom context

```js
export default {
  context: 'globalThis',
  output: {
    format: 'iife',
  },
};
```

### Use window for browser builds

```js
export default {
  context: 'window',
  platform: 'browser',
  output: {
    format: 'iife',
  },
};
```

## In-depth

The `context` option controls what `this` refers to in the top-level scope of your bundled code. This is particularly important for:

- **IIFE bundles** that need to access global objects
- **UMD bundles** that run in multiple environments
- **Code that references `this` at the top level**

By default:

- For `'iife'` and `'umd'` formats on browser platform: `this` is `'window'`
- For `'iife'` and `'umd'` formats on node platform: `this` is `'global'`
- For ES modules: `this` is `undefined`

Using `'globalThis'` provides a cross-platform way to access the global object that works in both browser and Node.js environments.
