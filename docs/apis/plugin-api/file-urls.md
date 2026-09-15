# File URLs

To reference a file URL reference from within JS code, use the `import.meta.ROLLDOWN_FILE_URL_referenceId` replacement. This will generate code that resolves the emitted file relative to `import.meta.url` and assumes the `URL` global is available. This works out of the box for the `esm` format, and for the `cjs` format on the `node` platform where `import.meta.url` is [polyfilled](/in-depth/non-esm-output-formats#well-known-import-meta-properties). For the `iife` and `umd` formats, `import.meta.url` needs to be polyfilled or the [`resolveFileUrl`](/reference/Interface.Plugin#resolvefileurl) hook needs to be implemented to return code that does not rely on `import.meta.url`. The same hook can also be used to customize the URL resolution for the other formats.

> [!TIP]
> Rolldown also accepts `import.meta.ROLLUP_FILE_URL_referenceId` as an alias of `import.meta.ROLLDOWN_FILE_URL_referenceId` for compatibility with Rollup.

The following example will detect imports of `.svg` files, emit the imported files as assets, and return their URLs to be used e.g. as the `src` attribute of an `img` tag:

::: code-group

```js [rolldown-plugin-svg-asset.js]
import path from 'node:path';
import fs from 'node:fs';

function svgResolverPlugin() {
  return {
    name: 'svg-resolver',
    resolveId: {
      filter: { id: /\.svg$/ },
      handler(source, importer) {
        return path.resolve(path.dirname(importer), source);
      },
    },
    load: {
      filter: { id: /\.svg$/ },
      handler(id) {
        const referenceId = this.emitFile({
          type: 'asset',
          name: path.basename(id),
          source: fs.readFileSync(id),
        });
        return `export default import.meta.ROLLDOWN_FILE_URL_${referenceId};`;
      },
    },
  };
}
```

```js [main.js (usage)]
import logo from '../images/logo.svg';
const image = document.createElement('img');
image.src = logo;
document.body.appendChild(image);
```

:::

Similar to assets, emitted chunks can be referenced from within JS code via `import.meta.ROLLDOWN_FILE_URL_referenceId` as well.

The following example will detect imports prefixed with `register-paint-worklet:` and generate the necessary code and separate chunk to generate a CSS paint worklet. Note that this will only work in modern browsers and will only work if the output format is set to `es`.

::: code-group

```js [rolldown-plugin-paint-worklet.js]
import { prefixRegex } from '@rolldown/pluginutils';
const REGISTER_WORKLET = 'register-paint-worklet:';

function registerPaintWorkletPlugin() {
  return {
    name: 'register-paint-worklet',
    load: {
      filter: { id: prefixRegex(REGISTER_WORKLET) },
      handler(id) {
        return `CSS.paintWorklet.addModule(
          import.meta.ROLLDOWN_FILE_URL_${this.emitFile({
            type: 'chunk',
            id: id.slice(REGISTER_WORKLET.length),
          })}
        );`;
      },
    },
    resolveId: {
      filter: { id: prefixRegex(REGISTER_WORKLET) },
      handler(source, importer) {
        // We remove the prefix, resolve everything to absolute ids and
        // add the prefix again. This makes sure that you can use
        // relative imports to define worklets
        return this.resolve(source.slice(REGISTER_WORKLET.length), importer).then(
          (resolvedId) => REGISTER_WORKLET + resolvedId.id,
        );
      },
    },
  };
}
```

```js [main.js (usage)]
import 'register-paint-worklet:./worklet.js';
import { color, size } from './config.js';
document.body.innerHTML += `<h1 style="background-image: paint(vertical-lines);">color: ${color}, size: ${size}</h1>`;
```

```js [worklet.js (usage)]
import { color, size } from './config.js';
registerPaint(
  'vertical-lines',
  class {
    paint(ctx, geom) {
      for (let x = 0; x < geom.width / size; x++) {
        ctx.beginPath();
        ctx.fillStyle = color;
        ctx.rect(x * size, 0, 2, geom.height);
        ctx.fill();
      }
    }
  },
);
```

```js [config.js (usage)]
export const color = 'greenyellow';
export const size = 6;
```

:::

If you build this code, both the main chunk and the worklet will share the code from `config.js` via a shared chunk. This enables us to make use of the browser cache to reduce transmitted data and speed up loading the worklet.

## Passing a `urlId`

::: warning Experimental

The `urlId` API is experimental and may change in minor versions.

:::

Rolldown extends the syntax with an optional `urlId` (`import.meta.ROLLDOWN_FILE_URL_referenceId_urlId`). The `urlId` is an arbitrary identifier that is forwarded to the [`resolveFileUrl`](/reference/Interface.Plugin#resolvefileurl) hook as `args.urlId`, so a single plugin can resolve the same emitted file differently depending on where it is referenced from:

```js [rolldown-plugin-svg-resolver.js]
import path from 'node:path';
import fs from 'node:fs';

function svgResolverPlugin() {
  return {
    name: 'svg-resolver',
    load: {
      filter: { id: /\.svg$/ },
      handler(id) {
        const referenceId = this.emitFile({
          type: 'asset',
          name: path.basename(id),
          source: fs.readFileSync(id),
        });
        // Append a `urlId` so `resolveFileUrl` can special-case this reference.
        return `export default import.meta.ROLLDOWN_FILE_URL_${referenceId}_inline;`;
      },
    },
    resolveFileUrl({ referenceId, relativePath, urlId }) {
      if (urlId === 'inline') {
        // resolve inlined references differently
      }
      // ...
    },
  };
}
```

The `urlId` is only recognized on the rolldown-specific `ROLLDOWN_FILE_URL_` prefix. The Rollup-compatible `ROLLUP_FILE_URL_` alias never carries one. The default resolution (when no plugin handles the reference) ignores `urlId`.

The `urlId` can only contain ASCII identifier characters: letters (`a`-`z`, `A`-`Z`), digits (`0`-`9`), `_`, and `$`.
