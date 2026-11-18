import { describe, expect, test } from 'vitest';
import { editFile, page, serverUrl, waitForBuildStable } from '~utils';

describe('lazy-compilation: late-lazy-chunk', () => {
  test('a lazy chunk rendered before an edit does not bring back the old code', async () => {
    let releaseB!: () => void;
    const bReleased = new Promise<void>((resolve) => (releaseB = resolve));
    let bRendered!: () => void;
    const bRenderedPromise = new Promise<void>((resolve) => (bRendered = resolve));
    await page.route(
      (url) => url.pathname === '/@vite/lazy' && url.search.includes('route-b.js'),
      async (route) => {
        const response = await route.fetch();
        bRendered();
        await bReleased;
        await route.fulfill({ response });
      },
    );

    try {
      await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
      await waitForBuildStable();

      await page.click('#late-lazy-chunk-b-btn');
      await bRenderedPromise;

      await page.click('#late-lazy-chunk-a-btn');
      await expect.poll(() => page.textContent('#late-lazy-chunk-lib')).toBe('lib-v1:value-v1');
      await waitForBuildStable();

      editFile('late-lazy-chunk/lib.js', (code) => code.replace('lib-v1', 'lib-v2'));
      await expect.poll(() => page.textContent('#late-lazy-chunk-lib')).toBe('lib-v2:value-v1');
      await waitForBuildStable();

      releaseB();
      await expect.poll(() => page.textContent('#late-lazy-chunk-b')).toBe('b-loaded');
      await waitForBuildStable();

      editFile('late-lazy-chunk/value.js', (code) => code.replace('value-v1', 'value-v2'));
      await expect.poll(() => page.textContent('#late-lazy-chunk-lib')).toBe('lib-v2:value-v2');
    } finally {
      releaseB();
      await page.unrouteAll({ behavior: 'ignoreErrors' });
    }
  });
});
