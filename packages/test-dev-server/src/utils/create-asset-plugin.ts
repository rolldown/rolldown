import nodeFsp from 'node:fs/promises';
import nodePath from 'node:path';
import type { Plugin } from 'rolldown';

// Vite's `DEFAULT_ASSETS_RE` (a representative subset). Imports of files with
// these extensions are emitted as hashed assets and resolved to a served URL.
const ASSET_RE =
  /\.(?:png|jpe?g|jfif|pjpeg|pjp|gif|svg|ico|webp|avif|cur|apng|bmp|mp4|webm|ogg|mp3|wav|flac|aac|m4a|opus|woff2?|eot|ttf|otf|webmanifest|wasm|pdf|txt)$/;

function cleanUrl(url: string): string {
  // strip `#hash` then `?query`
  return url.replace(/#[^?]*$/, '').replace(/\?.*$/s, '');
}

/**
 * A TS port of the **bundled-dev branch** of Vite's `vite:asset` plugin
 * (`packages/vite/src/node/plugins/asset.ts`).
 *
 * For an asset import like `import url from './img.png'`, the URL is resolved
 * EAGERLY in `load` via `emitFile` + `getFileName` — there is no
 * `__VITE_ASSET__` / `__ROLLDOWN_ASSET__` placeholder and therefore no reliance
 * on the `renderChunk` hook. That matters for full bundle mode: HMR patches and
 * lazy compiles never run `renderChunk`, so a placeholder would leak; resolving
 * at `load` means the module code carries the real `/assets/<name>-<hash>.ext`
 * URL in every path (initial build, HMR patch, lazy chunk).
 *
 * The emitted asset *bytes* reach the dev server via the dev engine's
 * `onAdditionalAssets` callback (HMR / lazy) or `onOutput` (full build), which
 * writes them into `memoryFiles`. See vitejs/vite#22596.
 *
 * Only the bundled-dev path is ported — no production-build placeholder scheme,
 * no `renderBuiltUrl`, no relative base, no data-URL inlining.
 */
export function createAssetPlugin(): Plugin {
  return {
    name: 'rolldown-dev-server:asset',
    async load(id) {
      const file = cleanUrl(id);
      if (!ASSET_RE.test(file)) {
        return null;
      }
      const source = await nodeFsp.readFile(file);
      // Re-bundle when the asset's bytes change (Vite parity).
      this.addWatchFile(file);
      const referenceId = this.emitFile({
        type: 'asset',
        name: nodePath.basename(file),
        originalFileName: file,
        source,
      });
      // In bundled dev the output layout is deterministic, so the final hashed
      // filename is known the instant the file is emitted — no `renderChunk`.
      const fileName = this.getFileName(referenceId);
      return {
        code: `export default ${JSON.stringify(`/${fileName}`)}`,
        moduleType: 'js',
      };
    },
  };
}
