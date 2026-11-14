import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// https://github.com/vitejs/vite/issues/22596
// An asset that a lazily-loaded module pulls in (here a JS-imported image) must
// usably load the first time that module is opened, not only after a refresh.
//
// In rolldown's native asset path this currently fails: the lazy chunk's patch
// is produced by the HMR codegen, which never runs the `renderChunk` hook —
// and that hook is where the builtin asset plugin rewrites the
// `__ROLLDOWN_ASSET__#<refId>` placeholder into the real hashed filename. So
// the patch ships the raw placeholder, the browser requests
// `/__ROLLDOWN_ASSET__#...`, and the image never decodes until a refresh serves
// the fully-generated bundle. (A second, latent failure is that the asset bytes
// are registered for serving only by the follow-up rebuild's `onOutput`.)
describe('lazy-compilation: emitted-asset', () => {
  test('serves a lazily-emitted asset on the first load (#vite-22596)', async () => {
    // Capture any failed asset response (a 404 on the hashed image URL).
    // Navigation happens here, not in setup (serve.ts skips it), so the server
    // sees a cold first request for this scenario.
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
      await waitForBuildStable();

      // 1. The scenario's setup ran (its button listener is attached).
      await expect.poll(() => page.textContent('#emitted-asset-status')).toBe('ready');

      // 2. Trigger the lazy import and wait for the module to run.
      await page.click('#emitted-asset-btn');
      await expect.poll(() => page.textContent('#emitted-asset-status')).toBe('loaded');

      // 3. The lazily-imported image must actually decode (`naturalWidth > 0`),
      //    which only happens if its asset URL was resolved and served on the
      //    first request instead of staying a placeholder / returning a 404.
      await expect
        .poll(() =>
          page
            .$eval(
              '#emitted-asset-image',
              (img: HTMLImageElement) => img.complete && img.naturalWidth > 0,
            )
            .catch(() => false),
        )
        .toBe(true);

      // 4. No asset request 404'd on the first load.
      expect(failedAssetResponses).toEqual([]);
    } finally {
      page.off('response', onResponse);
    }
  });
});
