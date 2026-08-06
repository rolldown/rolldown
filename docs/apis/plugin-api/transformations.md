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

Transform a chunk in [`renderChunk`](/reference/Interface.Plugin#renderchunk) and return the new map with it. Rolldown composes that map with the chunk's own, rebuilds `x_google_ignoreList`, and hashes the transformed code:

```js
import MagicString from 'magic-string';

renderChunk(code) {
  const s = new MagicString(code);
  s.prepend('/* banner */\n');
  return { code: s.toString(), map: s.generateMap({ hires: 'boundary' }) };
}
```

We discourage transforming in [`generateBundle`](/reference/Interface.Plugin#generatebundle). It runs after hashing, so the emitted filename keeps the hash of the untransformed code. It also runs after the `.map` asset is built, so editing `chunk.map` does not change that file. That said, if you have to transform there, compose the maps and write the asset yourself:

```js
import remapping from '@jridgewell/remapping';
import MagicString from 'magic-string';

generateBundle(options, bundle) {
  for (const chunk of Object.values(bundle)) {
    if (chunk.type !== 'chunk') continue;

    const s = new MagicString(chunk.code);
    // ...your transform...
    if (!s.hasChanged()) continue;

    // A low-resolution map can compose down to nothing, so keep the boundaries.
    const step = s.generateMap({ source: chunk.fileName, hires: 'boundary' });
    chunk.code = s.toString();

    if (chunk.map) {
      // Assign the composed map, do not spread it. `toString()` lives on its
      // prototype, and spreading would leave you with `[object Object]`.
      chunk.map = remapping([step, chunk.map], () => null);

      // The emitted file comes from this asset, not from `chunk.map`.
      const asset = bundle[`${chunk.fileName}.map`];
      if (asset) asset.source = chunk.map.toString();
    }
  }
}
```
