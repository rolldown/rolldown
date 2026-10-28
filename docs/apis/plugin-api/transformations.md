# Source Code Transformations

If a plugin transforms source code, it should generate a sourcemap automatically, unless there's a specific `sourceMap: false` option. Rolldown only cares about the `mappings` property (everything else is handled automatically). [magic-string](https://github.com/Rich-Harris/magic-string) provides a simple way to generate such a map for elementary transformations like adding or removing code snippets.

If it doesn't make sense to generate a sourcemap, return an empty sourcemap:

```js
return {
  code: transformedCode,
  map: { mappings: '' },
};
```

If the transformation does not move code, you can preserve existing sourcemaps by returning `null`:

```js
return {
  code: transformedCode,
  map: null,
};
```

## Transforming a Chunk

To transform a chunk, you can use [`renderChunk`](/reference/Interface.Plugin#renderchunk). If you return the sourcemap for the transform you applied, Rolldown composes that map with the previous transforms and rebuilds `x_google_ignoreList` field based on the options:

```js
import MagicString from 'magic-string';

export default function myPlugin() {
  return {
    name: 'example',
    renderChunk(code) {
      const s = new MagicString(code);
      s.prepend('/* banner */\n');
      return { code: s.toString(), map: s.generateMap({ hires: 'boundary' }) };
    },
  };
}
```

We discourage transforming in [`generateBundle`](/reference/Interface.Plugin#generatebundle). It runs after hashing, so the emitted filename keeps the hash of the untransformed code. It also runs after the `.map` asset is built, so editing `chunk.map` does not change that file. That said, if you have to transform there, compose the maps and write the asset yourself:

```js
import remapping from '@jridgewell/remapping';
import MagicString from 'magic-string';

export default function myPlugin() {
  return {
    name: 'example',
    generateBundle(options, bundle) {
      for (const chunk of Object.values(bundle)) {
        if (chunk.type !== 'chunk') continue;

        const s = new MagicString(chunk.code);
        // ...your transform...
        if (!s.hasChanged()) continue;

        // A low-resolution map can compose down to nothing, so keep the mappings at the boundaries.
        const step = s.generateMap({ source: chunk.fileName, hires: 'boundary' });
        chunk.code = s.toString();

        if (chunk.map) {
          // compose the sourcemap
          chunk.map = remapping([step, chunk.map], () => null);

          // The emitted file comes from this asset, not from `chunk.map`.
          const asset = bundle[`${chunk.fileName}.map`];
          if (asset) asset.source = chunk.map.toString();
        }
      }
    },
  };
}
```
