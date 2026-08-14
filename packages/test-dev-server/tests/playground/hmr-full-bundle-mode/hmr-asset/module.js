// Used by __tests__/hmr-full-bundle-mode.spec.ts ("HMR adds an asset import").
// Initially this module imports no asset. The spec edits it to add a
// JS-imported image and point an <img> at it, which lands as an HMR patch.
//
// That patch is produced by the HMR codegen, which never runs the `renderChunk`
// hook — the hook where the builtin asset plugin rewrites the
// `__ROLLDOWN_ASSET__#<refId>` placeholder into the real hashed filename. So the
// patch ships the raw placeholder (and the bytes aren't served either, since an
// HMR patch runs no generate), and the image fails to load until a full reload.
// Same root cause as the lazy-compilation `emitted-asset` scenario. See
// vitejs/vite#22596.
const slot = document.querySelector('.hmr-asset');
const img = document.createElement('img');
img.id = 'hmr-asset-image';
img.alt = 'hmr-asset';
slot.replaceChildren(img);
/* @asset-src */

import.meta.hot?.accept();
