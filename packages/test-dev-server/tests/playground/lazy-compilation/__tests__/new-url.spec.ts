import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { page, serverUrl } from '~utils';

// https://github.com/rolldown/rolldown/issues/9817 (sibling of #9812)
// `emitted-asset` covers `import x from './img'` (the `__ROLLDOWN_ASSET__`
// placeholder, resolved in `renderChunk`). This covers `new URL('./img',
// import.meta.url)` — a distinct path resolved in the module finalizer (link
// phase), which the HMR/lazy codegen's separate finalizer does NOT run. It must
// resolve to the hashed asset on the first lazy compile, not stay the raw
// specifier (which the browser resolves against the patch URL → 404).
//
// CURRENTLY RED on purpose: this is the committed reproduction for #9817. It
// flips green once the HMR/lazy codegen resolves `new URL` asset references.
describe('lazy-compilation: new-url', () => {
  test('resolves a `new URL` asset reference on the first load (#9817)', async () => {
    const failedAssetResponses: string[] = [];
    const onResponse = (res: Response) => {
      const url = res.url();
      if (/\.png(?:\?|$)/.test(url) && res.status() >= 400) {
        failedAssetResponses.push(`${res.status()} ${url}`);
      }
    };
    page.on('response', onResponse);

    try {
      await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
      await expect.poll(() => page.textContent('#new-url-status')).toBe('ready');

      await page.click('#new-url-btn');
      await expect.poll(() => page.textContent('#new-url-status')).toBe('loaded');

      // The `new URL` asset must actually decode (`naturalWidth > 0`), which
      // only happens if its reference was rewritten to the served hashed URL
      // rather than left as the raw `./new-url-image.png` specifier.
      await expect
        .poll(() =>
          page
            .$eval(
              '#new-url-image',
              (img: HTMLImageElement) => img.complete && img.naturalWidth > 0,
            )
            .catch(() => false),
        )
        .toBe(true);

      expect(failedAssetResponses).toEqual([]);
    } finally {
      page.off('response', onResponse);
    }
  });
});
