#### In-depth (`type: 'chunk'`)

If the `type` is `'chunk'`, this emits a new chunk with the given module `id` as entry point. This will not result in duplicate modules in the graph, instead if necessary, existing chunks will be split or a facade chunk with reexports will be created. Chunks with a specified [`fileName`](/reference/Interface.EmittedChunk#filename) will always generate separate chunks while other emitted chunks may be deduplicated with existing chunks even if the name does not match. If such a chunk is not deduplicated, the [`output.chunkFileNames`](/reference/OutputOptions.chunkFileNames) pattern will be used.

You can reference the URL of an emitted file in any code returned by a [`load`](/reference/Interface.Plugin#load) or [`transform`](/reference/Interface.Plugin#transform) plugin hook via `import.meta.ROLLUP_FILE_URL_referenceId` (returns a string). See [File URLs](/apis/plugin-api/file-urls) for more details and an example.

You can use [`this.getFileName(referenceId)`](/reference/Interface.PluginContext#getfilename) to determine the file name as soon as it is available. If the file name is not set explicitly, then:

- asset file names are available starting with the [`renderStart`](/reference/Interface.Plugin#renderstart) hook. For assets that are emitted later, the file name will be available immediately after emitting the asset.
- chunk file names that do not contain a hash are available as soon as chunks are created after the [`renderStart`](/reference/Interface.Plugin#renderstart) hook.
- if a chunk file name would contain a hash, using [`getFileName`](/reference/Interface.PluginContext#getfilename) in any hook before [`generateBundle`](/reference/Interface.Plugin#generatebundle) will return a name containing a placeholder instead of the actual name. If you use this file name or parts of it in a chunk you transform in [`renderChunk`](/reference/Interface.Plugin#renderchunk), Rolldown will replace the placeholder with the actual hash before [`generateBundle`](/reference/Interface.Plugin#generatebundle), making sure the hash reflects the actual content of the final generated chunk including all referenced file hashes.

#### In-depth (`type: 'prebuilt-chunk'`)

If the `type` is `'prebuilt-chunk'`, this emits a chunk with fixed contents provided by the [`code`](/reference/Interface.EmittedPrebuiltChunk#code) property.

To reference a prebuilt chunk in imports, we need to mark the "module" as external in the [`resolveId`](/reference/Interface.Plugin#resolveid) hook as prebuilt chunks are not part of the module graph. Instead, they behave like assets with chunk meta-data:

```js
function emitPrebuiltChunkPlugin() {
  return {
    name: 'emit-prebuilt-chunk',
    resolveId: {
      filter: { id: /^\.\/my-prebuilt-chunk\.js$/ },
      handler(source) {
        return {
          id: source,
          external: true,
        };
      },
    },
    buildStart() {
      this.emitFile({
        type: 'prebuilt-chunk',
        fileName: 'my-prebuilt-chunk.js',
        code: 'export const foo = "foo"',
        exports: ['foo'],
      });
    },
  };
}
```

Then you can reference the prebuilt chunk in your code by `import { foo } from './my-prebuilt-chunk.js';`.

#### In-depth (`type: 'asset'`)

If the `type` is `'asset'`, this emits an arbitrary new file with the given source as content. Assets with a specified [`fileName`](/reference/Interface.EmittedAsset#filename) will always generate separate files while other emitted assets may be deduplicated with existing assets if they have the same source even if the name does not match. If an asset without a [`fileName`](/reference/Interface.EmittedAsset#filename) is not deduplicated, the [`output.assetFileNames`](/reference/OutputOptions.assetFileNames) pattern will be used.
