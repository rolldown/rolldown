:::warning

When using `output.footer` with minification enabled, the footer content may be stripped out unless it is formatted as a legal comment. To ensure your footer persists through minification, do either:

- Use [`output.postFooter`](/reference/OutputOptions.postFooter) instead, which is added after minification, or
- Use one of these comment formats:
  - Comments starting with `/*!` (e.g., `/*! My footer */`)
  - Comments containing `@license` (e.g., `/* @license My footer */`)
  - Comments containing `@preserve` (e.g., `/* @preserve My footer */`)
  - Comments starting with `//!` (for single-line comments)

The latter way's behavior is controlled by the [`output.legalComments`](/reference/OutputOptions.legalComments) option, which defaults to `'inline'` and preserves these special comment formats.

:::

#### Examples

##### Expose the default export as `module.exports` for CJS output with all named exports as properties

```js
export default {
  output: {
    format: 'cjs',
    exports: 'named',
    footer: (chunk) => {
      if (chunk.isEntry) {
        return `
module.exports = exports.default;
module.exports.default = module.exports;
module.exports.foo = module.exports.default.foo;`;
      }
      return '';
    },
  },
};
```
