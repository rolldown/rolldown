Allows customizing how Rolldown resolves URLs of files that were emitted by plugins via [`this.emitFile`](/reference/Interface.PluginContext#emitfile). By default, Rolldown will generate code for `import.meta.ROLLUP_FILE_URL_referenceId` that resolves the emitted file relative to `import.meta.url`. This generates correct absolute URLs for the `esm` format, and for the `cjs` format on the `node` platform where `import.meta.url` is [polyfilled](/in-depth/non-esm-output-formats#well-known-import-meta-properties). For the `iife` and `umd` formats, `import.meta.url` is not available and the generated code will not work — Rolldown emits a warning in that case. To support these formats, this hook needs to be implemented to return code that does not rely on `import.meta.url`. See [File URLs](/apis/plugin-api/file-urls) for more details and an example.

This hook can be used to customize the behavior of `import.meta.ROLLUP_FILE_URL_referenceId`.

The returned string must be a single JavaScript expression. Also the returned expression must be side-effect free. If the URL is not used in the code, Rolldown will remove it.

::: tip `import.meta.url` in the returned string

If the returned string contains `import.meta.url`, it will be rewritten for non-ESM formats similarly to [when `import.meta.url` is used in the code directly](/in-depth/non-esm-output-formats#well-known-import-meta-properties). Unlike Rolldown, Rollup outputs `import.meta.url` as-is.

:::

#### Example

The following plugin will always resolve all files relative to the current document:

```js
function resolveToDocumentPlugin() {
  return {
    name: 'resolve-to-document',
    resolveFileUrl({ fileName }) {
      return `new URL(${JSON.stringify(fileName)}, document.baseURI).href`;
    },
  };
}
```
