# File URLs

To reference a file URL reference from within JS code, use the `import.meta.ROLLUP_FILE_URL_referenceId` replacement. This will generate code that depends on the output format and generates a URL that points to the emitted file in the target environment. Note that the transformation assumes `URL` is available and `import.meta.url` is polyfilled except for CJS and ESM output formats.

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
        return `export default import.meta.ROLLUP_FILE_URL_${referenceId};`;
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

Similar to assets, emitted chunks can be referenced from within JS code via `import.meta.ROLLUP_FILE_URL_referenceId` as well.

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
          import.meta.ROLLUP_FILE_URL_${this.emitFile({
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
